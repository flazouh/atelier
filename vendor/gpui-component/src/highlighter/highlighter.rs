#[cfg(test)]
use crate::highlighter::HighlightTheme;
use crate::highlighter::LanguageRegistry;
use crate::highlighter::GrammarConfig;

use anyhow::{Context, Result, anyhow};
use gpui::{HighlightStyle, SharedString};
use gpui_base::input::RopeExt as _;
use ropey::{ChunkCursor, Rope};
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::{
    collections::{BTreeSet, HashMap},
    ops::{ControlFlow, Range},
    usize,
};
use sum_tree::Bias;
use tree_sitter::{
    InputEdit, ParseOptions, Parser, Point, Query, QueryCursor, StreamingIterator, Tree,
};

const MAX_INJECTION_RANGES: usize = 4096;
const MAX_INJECTION_BYTES: usize = 512 * 1024;
const MAX_INJECTION_LANGUAGE_BYTES: usize = 64;
/// Parse attempts, not resulting layers: a failed parse still spends budget.
/// Matches past it keep host highlighting but get no injected tokens.
const MAX_NON_COMBINED_INJECTION_PARSES: usize = 512;
/// How long one injection layer's parse may take, unless [`SyntaxHighlighter::set_injection_budget`]
/// says otherwise.
const INJECTION_PARSE_TIMEOUT: Duration = Duration::from_millis(20);

/// A syntax highlighter that supports incremental parsing, multiline text,
/// and caching of highlight results.
#[allow(unused)]
pub struct SyntaxHighlighter {
    language: SharedString,
    query: Option<Arc<Query>>,
    /// The full injections query. This is used to build injection layers during parsing.
    injections_query: Option<Arc<Query>>,

    locals_pattern_index: usize,
    highlights_pattern_index: usize,
    // highlight_indices: Vec<Option<Highlight>>,
    non_local_variable_patterns: Vec<bool>,
    injection_content_capture_index: Option<u32>,
    injection_language_capture_index: Option<u32>,
    local_scope_capture_index: Option<u32>,
    local_def_capture_index: Option<u32>,
    local_def_value_capture_index: Option<u32>,
    local_ref_capture_index: Option<u32>,

    /// The last parsed source text.
    text: Rope,
    parser: Parser,
    /// The last parsed tree.
    tree: Option<Tree>,

    /// Parsed injection trees.
    /// These are built once in update() and queried multiple times in match_styles().
    injection_layers: Vec<InjectionLayer>,
    /// Each combined injection's ranges before `normalize_combined_injection_ranges`, so an
    /// edit can rebuild its layer without querying the whole tree.
    combined_ranges: Vec<(SharedString, Vec<tree_sitter::Range>)>,
    /// Whether the last injection pass stopped at `MAX_NON_COMBINED_INJECTION_PARSES`.
    injections_capped: bool,
    /// Whether the last update changed the injection layers in place, for tests.
    injections_edited: bool,
    /// How long one injection layer's parse may take; `None` for as long as it needs.
    injection_budget: Option<Duration>,
    /// The new text that edits since the last whole set of layers wrote, while the layers moved
    /// with each one (`edit_tree`); `None` when the layers were not whole to begin with.
    moved_since_parse: Option<Range<usize>>,
    /// Whether `injection_layers` match `text`, so the next edit can update them in place.
    /// False after a parse that skipped them (a timeout, `edit_tree`) or a pass that dropped
    /// combined ranges at a cap.
    injections_current: bool,
}

/// A parsed injection layer.
/// Stores the parsed tree and the ranges it covers.
#[derive(Clone)]
pub(crate) struct InjectionLayer {
    pub(crate) language_name: SharedString,
    highlight_query: Arc<Query>,
    pub(crate) ranges: Vec<tree_sitter::Range>,
    pub(crate) byte_range: Range<usize>,
    pub(crate) tree: Tree,
    /// One layer over all of a language's ranges (`injection.combined`).
    combined: bool,
    /// The bytes of every node the match captured, the language and predicate nodes too:
    /// an edit there can change the layer though its ranges stay the same.
    match_range: Range<usize>,
    /// Moved by an edit since it was parsed: its tree shows where the old colours went, but a
    /// combined layer's tree parsed again on it would not match a fresh parse, so it is not reused.
    followed: bool,
}

/// Updates injection `layers` for a change in place: keeps each layer the change did not touch,
/// moved to its new offsets, and queries and parses again only where `old_tree` (already edited)
/// and `new_tree` differ or the text changed. The result equals a full
/// [`SyntaxHighlighter::compute_injection_layers`]; `None` when only a full pass can be exact.
#[allow(clippy::too_many_arguments)]
fn update_layers(
    data: &InjectionParseData,
    layers: &[InjectionLayer],
    old_combined: &[(SharedString, Vec<tree_sitter::Range>)],
    capped_before: bool,
    moved: &Moved,
    old_tree: &Tree,
    new_tree: &Tree,
    text: &Rope,
) -> Option<InjectionLayers> {
        let text_len = text.len();
        // Where the tree or the text changed, in new offsets, a byte wider on each side so an
        // empty range (a deletion) still meets the nodes around it.
        let mut region: Vec<Range<usize>> = old_tree
            .changed_ranges(new_tree)
            .map(|r| r.start_byte..r.end_byte)
            .chain([moved.span()])
            .map(|r| r.start.saturating_sub(1)..(r.end + 1).min(text_len))
            .collect();
        region.sort_by_key(|r| r.start);
        let mut merged: Vec<Range<usize>> = Vec::with_capacity(region.len());
        for range in region {
            match merged.last_mut() {
                Some(last) if range.start <= last.end => last.end = last.end.max(range.end),
                _ => merged.push(range),
            }
        }
        let region = merged;
        let touches = |r: &Range<usize>| region.iter().any(|g| r.start <= g.end && g.start <= r.end);

        let mut highlight_queries: HashMap<SharedString, Arc<Query>> = layers
            .iter()
            .map(|layer| (layer.language_name.clone(), layer.highlight_query.clone()))
            .collect();
        let mut resolved_languages = HashMap::new();
        // A layer that only touches the region is dropped, and the query needs a node to
        // overlap its range, so it runs a byte wider still and finds that layer again.
        let mut found = Vec::new();
        for range in &region {
            found.extend(find_injections(
                data,
                new_tree,
                text,
                Some(range.start.saturating_sub(1)..(range.end + 1).min(text_len)),
                &mut highlight_queries,
                &mut resolved_languages,
            ));
        }

        // Single layers the edit did not touch, moved. Combined ones are parsed again below.
        let mut kept = Vec::new();
        let mut timed_out = false;
        for layer in layers {
            if layer.combined {
                continue;
            } else if let Some(moved) = moved
                .layer(layer)
                .filter(|l| !touches(&l.byte_range) && !touches(&l.match_range))
            {
                kept.push(moved);
            }
        }

        // Single injections found in the region: new, or replacing a kept layer they overlap.
        let mut singles: Vec<FoundInjection> = Vec::new();
        let mut combined_found: HashMap<SharedString, Vec<tree_sitter::Range>> = HashMap::new();
        for injection in found {
            if injection.combined {
                let ranges = combined_found.entry(injection.language_name).or_default();
                for range in injection.ranges {
                    if !ranges.contains(&range) {
                        ranges.push(range);
                    }
                }
                continue;
            }
            if !injection_ranges_within_limits(&injection.ranges)
                || singles.iter().any(|s| {
                    s.language_name == injection.language_name && s.ranges == injection.ranges
                })
            {
                continue;
            }
            if kept.iter().any(|k| {
                k.language_name == injection.language_name && k.ranges == injection.ranges
            }) {
                continue;
            }
            if let Some(envelope) = bounding_byte_range(&injection.ranges) {
                kept.retain(|k| !(k.byte_range.start <= envelope.end && envelope.start <= k.byte_range.end));
            }
            singles.push(injection);
        }
        singles.sort_by_key(|s| s.ranges[0].start_byte);

        // A full pass keeps the first layers in text order up to the cap.
        let total = kept.len() + singles.len();
        if capped_before && total < MAX_NON_COMBINED_INJECTION_PARSES {
            // Layers past the old cap would come back, and they were never parsed.
            return None;
        }
        let capped = capped_before || total > MAX_NON_COMBINED_INJECTION_PARSES;
        let (mut k, mut n) = (0, 0);
        while k + n < MAX_NON_COMBINED_INJECTION_PARSES && (k < kept.len() || n < singles.len()) {
            let kept_first = match (kept.get(k), singles.get(n)) {
                (Some(a), Some(b)) => a.byte_range.start <= b.ranges[0].start_byte,
                (Some(_), None) => true,
                _ => false,
            };
            if kept_first {
                k += 1;
            } else {
                n += 1;
            }
        }
        kept.truncate(k);
        singles.truncate(n);

        // Combined layers: their ranges, moved and updated, then parsed afresh. Parsing on the
        // old tree edited does not match a fresh parse once the included ranges change.
        let mut languages: Vec<SharedString> = old_combined
            .iter()
            .map(|(language, _)| language.clone())
            .chain(combined_found.keys().cloned())
            .collect();
        languages.sort_by(|a, b| a.as_ref().cmp(b.as_ref()));
        languages.dedup();
        let mut combined_layers = Vec::new();
        let mut combined_ranges = Vec::new();
        for language_name in languages {
            let mut ranges: Vec<tree_sitter::Range> = old_combined
                .iter()
                .filter(|(language, _)| *language == language_name)
                .flat_map(|(_, ranges)| ranges.iter())
                .filter_map(|range| moved.range(range))
                .filter(|range| !touches(&(range.start_byte..range.end_byte)))
                .collect();
            for range in combined_found.remove(&language_name).unwrap_or_default() {
                if !ranges.contains(&range) {
                    ranges.push(range);
                }
            }
            if !injection_ranges_within_limits(&ranges) {
                return None;
            }
            if ranges.is_empty() {
                continue;
            }
            sort_ranges(&mut ranges);
            let normalized = normalize_combined_injection_ranges(&language_name, ranges.clone());
            if normalized.last() != ranges.last() {
                return None;
            }
            combined_ranges.push((language_name.clone(), ranges));
            let Some(highlight_query) = highlight_queries.get(&language_name).cloned() else {
                continue;
            };
            if let Some(layer) = SyntaxHighlighter::parse_injection_layer(
                &language_name,
                highlight_query,
                normalized,
                None,
                text,
                true,
                data.budget,
                &mut timed_out,
            ) {
                combined_layers.push(layer);
            }
        }

        for injection in singles {
            if let Some(layer) = SyntaxHighlighter::parse_injection_layer(
                &injection.language_name,
                injection.highlight_query,
                injection.ranges,
                None,
                text,
                false,
                data.budget,
                &mut timed_out,
            ) {
                kept.push(InjectionLayer {
                    match_range: injection.match_range,
                    ..layer
                });
            }
        }
        kept.extend(combined_layers);
        sort_layers(&mut kept);
        // A layer that ran out of time is missing, so the next edit builds them all again.
        Some(InjectionLayers { layers: kept, combined_ranges, capped, complete: !timed_out })
}



/// How the injection layers stand against the new text: moved here by one edit, or already moved
/// with each edit as it came (by [`SyntaxHighlighter::edit_tree`]), which touched `span` of the new
/// text in all.
enum Moved {
    By(InputEdit),
    Already(Range<usize>),
}

impl Moved {
    /// The new text the change wrote, where the layers may be stale.
    fn span(&self) -> Range<usize> {
        match self {
            Moved::By(edit) => edit.start_byte..edit.new_end_byte,
            Moved::Already(span) => span.clone(),
        }
    }

    /// `layer` in the new text's offsets; `None` when the edit touches it.
    fn layer(&self, layer: &InjectionLayer) -> Option<InjectionLayer> {
        match self {
            Moved::By(edit) => layer.edited(edit),
            Moved::Already(_) => Some(layer.clone()),
        }
    }

    /// A combined layer's raw range in the new text's offsets; `None` when the edit touches it.
    fn range(&self, range: &tree_sitter::Range) -> Option<tree_sitter::Range> {
        match self {
            Moved::By(edit) => shift_range(range, edit),
            Moved::Already(_) => Some(*range),
        }
    }
}

/// One parse at a time for an editor. A request while one runs waits as a single queued parse,
/// however many come, so the keystrokes during a parse coalesce into the next one.
#[derive(Clone, Copy, Debug, Default)]
pub struct ParseQueue {
    running: bool,
    queued: bool,
}

impl ParseQueue {
    /// A parse is wanted: `true` when it starts now, `false` when it waits behind the running one.
    pub fn request(&mut self) -> bool {
        if self.running {
            self.queued = true;
            return false;
        }
        self.running = true;
        true
    }

    /// The running parse ended: `true` when the queued one starts now.
    pub fn finish(&mut self) -> bool {
        if self.queued {
            self.queued = false;
            return true;
        }
        self.running = false;
        false
    }
}

/// A parse to run on another thread: see [`SyntaxHighlighter::background_parse`].
pub struct BackgroundParse {
    language: SharedString,
    old_tree: Option<Tree>,
    text: Rope,
    injections: Option<InjectionParseData>,
    /// The layers as the edits moved them, for the in-place update; `None` when it cannot apply.
    moved: Option<MovedLayers>,
}

/// Whole layers, moved with the edits since, and the span of new text those edits wrote.
struct MovedLayers {
    data: InjectionParseData,
    layers: Vec<InjectionLayer>,
    combined_ranges: Vec<(SharedString, Vec<tree_sitter::Range>)>,
    capped: bool,
    span: Range<usize>,
}

/// A finished background parse, for [`SyntaxHighlighter::apply_parsed`].
pub struct ParsedTree {
    tree: Tree,
    text: Rope,
    injections: InjectionLayers,
    in_place: bool,
}

impl ParsedTree {
    pub fn tree(&self) -> &Tree {
        &self.tree
    }

    /// Whether its injection layers were updated in place, not rebuilt: for tests and timings.
    #[doc(hidden)]
    pub fn injections_in_place(&self) -> bool {
        self.in_place
    }
}

impl BackgroundParse {
    /// Parses the text on the old tree, as edited, then builds its injection layers. No budget:
    /// it runs off the UI thread.
    pub fn run(self) -> Option<ParsedTree> {
        let (mut parser, grammar) = LanguageRegistry::singleton().parser(&self.language).ok()?;
        parser.set_language(&grammar).ok()?;
        let text = &self.text;
        let tree = parser.parse_with_options(
            &mut |offset, _| {
                if offset >= text.len() {
                    ""
                } else {
                    let (chunk, chunk_byte_ix) = text.chunk(offset);
                    &chunk[offset - chunk_byte_ix..]
                }
            },
            self.old_tree.as_ref(),
            None,
        )?;
        // In place when the layers were whole and moved with the edits; a full pass otherwise.
        let in_place = match (self.moved, self.old_tree.as_ref()) {
            (Some(m), Some(old_tree)) => update_layers(
                &m.data,
                &m.layers,
                &m.combined_ranges,
                m.capped,
                &Moved::Already(m.span),
                old_tree,
                &tree,
                text,
            ),
            _ => None,
        };
        let (injections, in_place) = match (in_place, self.injections) {
            (Some(layers), _) => (layers, true),
            (None, Some(data)) => (SyntaxHighlighter::compute_injection_layers(data, &tree, text), false),
            (None, None) => (InjectionLayers::default(), false),
        };
        Some(ParsedTree { tree, text: self.text, injections, in_place })
    }
}

/// Where a range's end lands after `edit`: before it, it stays; after it, it moves by the edit's
/// length change; inside the replaced text, a start goes to the end of the new text and an end to
/// the start of the edit, so the range keeps only what the edit left of it. Text typed exactly at a
/// range's start or end joins neither side: the start moves past it, the end stays before it.
fn follow_offset(offset: usize, edit: &InputEdit, start: bool) -> usize {
    if offset < edit.start_byte || (!start && offset == edit.start_byte) {
        offset
    } else if offset >= edit.old_end_byte {
        offset - edit.old_end_byte + edit.new_end_byte
    } else if start {
        edit.new_end_byte
    } else {
        edit.start_byte
    }
}

fn follow_point(point: Point, offset: usize, edit: &InputEdit, start: bool) -> Point {
    if offset < edit.start_byte || (!start && offset == edit.start_byte) {
        point
    } else if offset >= edit.old_end_byte {
        shift_point(point, edit)
    } else if start {
        edit.new_end_position
    } else {
        edit.start_position
    }
}

/// `range` after `edit`, as [`follow_offset`] moves its ends; an edit inside it makes it grow or
/// shrink with the text.
pub fn follow_range(range: &Range<usize>, edit: &InputEdit) -> Range<usize> {
    let start = follow_offset(range.start, edit, true);
    start..follow_offset(range.end, edit, false).max(start)
}

/// A tree-sitter range after `edit`, as [`follow_range`] moves byte ranges.
fn follow_ts_range(range: &tree_sitter::Range, edit: &InputEdit) -> tree_sitter::Range {
    let start_byte = follow_offset(range.start_byte, edit, true);
    let end_byte = follow_offset(range.end_byte, edit, false).max(start_byte);
    tree_sitter::Range {
        start_point: follow_point(range.start_point, range.start_byte, edit, true),
        end_point: follow_point(range.end_point, range.end_byte, edit, false),
        start_byte,
        end_byte,
    }
}

impl InjectionLayer {
    /// Moves this layer with `edit` without parsing: its tree is edited and its ranges follow.
    fn follow(&mut self, edit: &InputEdit) {
        if self.byte_range.end >= edit.start_byte {
            self.tree.edit(edit);
        }
        for range in &mut self.ranges {
            *range = follow_ts_range(range, edit);
        }
        if let Some(bytes) = bounding_byte_range(&self.ranges) {
            self.byte_range = bytes;
        }
        self.match_range = follow_range(&self.match_range, edit);
        self.followed = true;
    }

    /// This layer moved by `edit`, or `None` when the edit touches its match.
    fn edited(&self, edit: &InputEdit) -> Option<InjectionLayer> {
        let match_range = shift_bytes(&self.match_range, edit)?;
        let ranges = self
            .ranges
            .iter()
            .map(|range| shift_range(range, edit))
            .collect::<Option<Vec<_>>>()?;
        let mut tree = self.tree.clone();
        if self.byte_range.end >= edit.start_byte {
            tree.edit(edit);
        }
        Some(InjectionLayer {
            language_name: self.language_name.clone(),
            highlight_query: self.highlight_query.clone(),
            byte_range: bounding_byte_range(&ranges)?,
            ranges,
            tree,
            combined: self.combined,
            match_range,
            followed: self.followed,
        })
    }
}

/// Injection layers for a whole text, with what an edit needs to update them in place.
pub(crate) struct InjectionLayers {
    layers: Vec<InjectionLayer>,
    combined_ranges: Vec<(SharedString, Vec<tree_sitter::Range>)>,
    capped: bool,
    complete: bool,
}

impl Default for InjectionLayers {
    fn default() -> Self {
        Self {
            layers: Vec::new(),
            combined_ranges: Vec::new(),
            capped: false,
            complete: true,
        }
    }
}

/// An injection that one query match asks for, its ranges filtered and sorted.
struct FoundInjection {
    language_name: SharedString,
    highlight_query: Arc<Query>,
    ranges: Vec<tree_sitter::Range>,
    combined: bool,
    /// See [`InjectionLayer::match_range`].
    match_range: Range<usize>,
}

/// The ranges of one combined injection, up to the count and byte caps.
struct CombinedRanges {
    ranges: Vec<tree_sitter::Range>,
    byte_count: usize,
    /// Whether a cap dropped a range.
    truncated: bool,
}

impl CombinedRanges {
    /// Ranges are already filtered by `should_include_injection_range`
    /// before being pushed here; this only enforces the count/byte caps.
    fn push_limited(&mut self, ranges: Vec<tree_sitter::Range>) {
        for range in ranges {
            if self.ranges.len() >= MAX_INJECTION_RANGES {
                self.truncated = true;
                break;
            }

            let range_len = injection_range_len(&range);
            if self.byte_count.saturating_add(range_len) > MAX_INJECTION_BYTES {
                self.truncated = true;
                break;
            }

            self.byte_count += range_len;
            self.ranges.push(range);
        }
    }
}

fn bounding_byte_range(ranges: &[tree_sitter::Range]) -> Option<Range<usize>> {
    let start = ranges.iter().map(|r| r.start_byte).min()?;
    let end = ranges.iter().map(|r| r.end_byte).max()?;
    Some(start..end)
}

fn sort_ranges(ranges: &mut [tree_sitter::Range]) {
    ranges.sort_unstable_by(|a, b| {
        a.start_byte
            .cmp(&b.start_byte)
            .then_with(|| a.end_byte.cmp(&b.end_byte))
    });
}

fn ranges_cache_key(ranges: &[tree_sitter::Range]) -> Vec<(usize, usize)> {
    ranges.iter().map(|r| (r.start_byte, r.end_byte)).collect()
}

/// Layers in text order; ties break the same way on every pass.
fn sort_layers(layers: &mut [InjectionLayer]) {
    layers.sort_by(|a, b| {
        (a.byte_range.start, a.byte_range.end, a.language_name.as_ref())
            .cmp(&(b.byte_range.start, b.byte_range.end, b.language_name.as_ref()))
    });
}

/// `point` moved by `edit`; it must be after the edit's old end.
fn shift_point(point: Point, edit: &InputEdit) -> Point {
    if point.row == edit.old_end_position.row {
        Point::new(
            edit.new_end_position.row,
            point.column - edit.old_end_position.column + edit.new_end_position.column,
        )
    } else {
        Point::new(
            point.row - edit.old_end_position.row + edit.new_end_position.row,
            point.column,
        )
    }
}

/// `range` moved by `edit`, or `None` when the edit touches it.
fn shift_bytes(range: &Range<usize>, edit: &InputEdit) -> Option<Range<usize>> {
    if range.end < edit.start_byte {
        return Some(range.clone());
    }
    if range.start <= edit.old_end_byte {
        return None;
    }
    let delta = |offset: usize| offset - edit.old_end_byte + edit.new_end_byte;
    Some(delta(range.start)..delta(range.end))
}

/// `range` moved by `edit`, or `None` when the edit touches it.
fn shift_range(range: &tree_sitter::Range, edit: &InputEdit) -> Option<tree_sitter::Range> {
    if range.end_byte < edit.start_byte {
        return Some(*range);
    }
    if range.start_byte <= edit.old_end_byte {
        return None;
    }
    Some(tree_sitter::Range {
        start_byte: range.start_byte - edit.old_end_byte + edit.new_end_byte,
        end_byte: range.end_byte - edit.old_end_byte + edit.new_end_byte,
        start_point: shift_point(range.start_point, edit),
        end_point: shift_point(range.end_point, edit),
    })
}

fn resolve_language(
    language_name: &str,
    query_cache: &mut HashMap<SharedString, Arc<Query>>,
) -> Option<(SharedString, Arc<Query>)> {
    let config = LanguageRegistry::singleton().language(language_name)?;
    if let Some(query) = query_cache.get(&config.name) {
        return Some((config.name, query.clone()));
    }

    let grammar = LanguageRegistry::singleton().grammar(language_name).ok()?;
    let query = match Query::new(&grammar, &config.highlights) {
        Ok(query) => Arc::new(query),
        Err(error) => {
            tracing::error!(
                "failed to build injection query for {:?}: {:?}",
                config.name,
                error
            );
            return None;
        }
    };
    query_cache.insert(config.name.clone(), query.clone());
    Some((config.name, query))
}

/// The injections `data.query` finds in `tree`, in match order: all of them, or with
/// `byte_range`, those whose match touches it.
fn find_injections(
    data: &InjectionParseData,
    tree: &Tree,
    text: &Rope,
    byte_range: Option<Range<usize>>,
    highlight_queries: &mut HashMap<SharedString, Arc<Query>>,
    resolved_languages: &mut HashMap<SharedString, Option<(SharedString, Arc<Query>)>>,
) -> Vec<FoundInjection> {
    let mut cursor = QueryCursor::new();
    if let Some(byte_range) = byte_range {
        cursor.set_byte_range(byte_range);
    }
    let mut matches = cursor.matches(&data.query, tree.root_node(), TextProvider(text));
    let mut found = Vec::new();
    while let Some(query_match) = matches.next() {
        let mut language_name: Option<SharedString> = None;
        let mut combined = false;
        for prop in data.query.property_settings(query_match.pattern_index) {
            match prop.key.as_ref() {
                "injection.language" => {
                    language_name = prop
                        .value
                        .as_ref()
                        .map(|v| SharedString::from(v.to_string()));
                }
                "injection.combined" => combined = true,
                _ => {}
            }
        }

        if language_name.is_none() {
            language_name = query_match
                .captures
                .iter()
                .find(|cap| Some(cap.index) == data.language_capture_index)
                .and_then(|capture| captured_injection_language(text, capture.node.byte_range()));
        }

        let Some(raw_language_name) = language_name else {
            continue;
        };
        // Cache raw names as well as canonical queries. Otherwise every fence with the
        // same info string would lock the registry and clone its language configuration.
        let resolved_language = if let Some(resolved) = resolved_languages.get(&raw_language_name)
        {
            resolved.clone()
        } else {
            let resolved = resolve_language(&raw_language_name, highlight_queries);
            resolved_languages.insert(raw_language_name, resolved.clone());
            resolved
        };
        let Some((language_name, highlight_query)) = resolved_language else {
            continue;
        };

        let mut ranges = query_match
            .captures
            .iter()
            .filter(|cap| Some(cap.index) == data.content_capture_index)
            .map(|capture| capture.node.range())
            .collect::<Vec<_>>();
        ranges.retain(|range| should_include_injection_range(&language_name, range, text));
        if ranges.is_empty() {
            continue;
        }
        sort_ranges(&mut ranges);
        let start = query_match.captures.iter().map(|c| c.node.start_byte()).min();
        let end = query_match.captures.iter().map(|c| c.node.end_byte()).max();
        found.push(FoundInjection {
            language_name,
            highlight_query,
            ranges,
            combined,
            match_range: start.unwrap_or(0)..end.unwrap_or(0),
        });
    }
    found
}

/// Data needed to compute injection layers on a background thread.
pub(crate) struct InjectionParseData {
    /// How long one layer's parse may take; `None` for as long as it needs.
    pub(crate) budget: Option<Duration>,
    pub(crate) query: Arc<Query>,
    pub(crate) content_capture_index: Option<u32>,
    pub(crate) language_capture_index: Option<u32>,
    /// Old injection trees that can be reused when the injected ranges are unchanged.
    pub(crate) old_layers: Vec<ReusableInjectionLayer>,
}

pub(crate) struct ReusableInjectionLayer {
    pub(crate) language_name: SharedString,
    highlight_query: Arc<Query>,
    pub(crate) ranges: Vec<tree_sitter::Range>,
    pub(crate) tree: Tree,
}

struct TextProvider<'a>(&'a Rope);
struct ByteChunks<'a> {
    cursor: ChunkCursor<'a>,
    node_start: usize,
    node_end: usize,
    at_first: bool,
}
impl<'a> tree_sitter::TextProvider<&'a [u8]> for TextProvider<'a> {
    type I = ByteChunks<'a>;

    fn text(&mut self, node: tree_sitter::Node) -> Self::I {
        let range = node.byte_range();
        let cursor = self.0.chunk_cursor_at(range.start);

        ByteChunks {
            cursor,
            node_start: range.start,
            node_end: range.end,
            at_first: true,
        }
    }
}

impl<'a> Iterator for ByteChunks<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        if !self.at_first {
            if !self.cursor.next() {
                return None;
            }
        }
        self.at_first = false;

        let chunk_byte_start = self.cursor.byte_offset();
        if chunk_byte_start >= self.node_end {
            return None;
        }

        let chunk = self.cursor.chunk().as_bytes();

        // Slice the chunk to only include bytes within the node's range.
        let start_in_chunk = self.node_start.saturating_sub(chunk_byte_start);
        let end_in_chunk = (self.node_end - chunk_byte_start).min(chunk.len());

        if start_in_chunk >= end_in_chunk {
            return None;
        }

        Some(&chunk[start_in_chunk..end_in_chunk])
    }
}

fn injection_range_len(range: &tree_sitter::Range) -> usize {
    range.end_byte.saturating_sub(range.start_byte)
}

fn injection_ranges_byte_count(ranges: &[tree_sitter::Range]) -> usize {
    ranges.iter().map(injection_range_len).sum()
}

fn injection_ranges_within_limits(ranges: &[tree_sitter::Range]) -> bool {
    ranges.len() <= MAX_INJECTION_RANGES
        && injection_ranges_byte_count(ranges) <= MAX_INJECTION_BYTES
}

/// Read a captured injection language without ever allocating an unbounded
/// amount of source text. Language identifiers in fenced code blocks are tiny;
/// longer captures cannot name a registered language and are ignored.
fn captured_injection_language(text: &Rope, range: Range<usize>) -> Option<SharedString> {
    if range.end > text.len()
        || range.start >= range.end
        || range.end.saturating_sub(range.start) > MAX_INJECTION_LANGUAGE_BYTES
    {
        return None;
    }

    let language = text.slice(range).to_string();
    let language = language.trim();
    (!language.is_empty()).then(|| SharedString::from(language.to_string()))
}

/// Combined markdown inline injections are parsed as one tree with
/// `set_included_ranges`. If we include only the inline nodes that contain
/// trigger bytes, the parser sees those ranges as adjacent and can merge a
/// closing backtick from one list item with an opening backtick from the next.
/// Re-inserting the separator bytes between retained inline ranges preserves
/// the original boundaries.
///
/// The separator bytes count against the same `MAX_INJECTION_RANGES` /
/// `MAX_INJECTION_BYTES` budget as the content ranges, so on a very large
/// document the tail ranges may be dropped here even though `push_limited`
/// already admitted them.
fn normalize_combined_injection_ranges(
    language_name: &SharedString,
    ranges: Vec<tree_sitter::Range>,
) -> Vec<tree_sitter::Range> {
    if language_name.as_ref() != "markdown_inline" || ranges.len() <= 1 {
        return ranges;
    }

    let mut normalized = Vec::with_capacity(ranges.len().min(MAX_INJECTION_RANGES));
    let mut byte_count = 0usize;
    let mut previous_range: Option<tree_sitter::Range> = None;

    for range in ranges {
        let mut pending_ranges = Vec::with_capacity(2);
        if let Some(previous) = previous_range {
            if previous.end_byte < range.start_byte {
                pending_ranges.push(tree_sitter::Range {
                    start_byte: previous.end_byte,
                    end_byte: range.start_byte,
                    start_point: previous.end_point,
                    end_point: range.start_point,
                });
            }
        }
        pending_ranges.push(range);

        let pending_len = pending_ranges
            .iter()
            .map(injection_range_len)
            .sum::<usize>();
        if normalized.len().saturating_add(pending_ranges.len()) > MAX_INJECTION_RANGES
            || byte_count.saturating_add(pending_len) > MAX_INJECTION_BYTES
        {
            break;
        }

        byte_count += pending_len;
        normalized.extend(pending_ranges);
        previous_range = Some(range);
    }

    normalized
}

fn should_include_injection_range(
    language_name: &SharedString,
    range: &tree_sitter::Range,
    text: &Rope,
) -> bool {
    if language_name.as_ref() != "markdown_inline" {
        return true;
    }

    markdown_inline_range_has_trigger(text, range.start_byte..range.end_byte)
}

/// Returns whether an inline range contains any byte that could start a
/// Markdown inline construct, so plain prose ranges skip the injected parse.
///
/// The byte set must stay a superset of the trigger characters for every node
/// captured by `languages/markdown_inline/highlights.scm` (emphasis, code
/// spans, links, images, autolinks). If that query gains a construct with a new
/// trigger character (e.g. GFM bare autolinks), add it here or the construct
/// will silently lose highlighting.
fn markdown_inline_range_has_trigger(text: &Rope, range: Range<usize>) -> bool {
    text.slice(range).bytes().any(|byte| {
        matches!(
            byte,
            b'*' | b'_' | b'`' | b'[' | b']' | b'(' | b')' | b'<' | b'>' | b'!' | b'~' | b'$'
        )
    })
}

#[derive(Debug, Default, Clone)]
struct HighlightSummary {
    count: usize,
    start: usize,
    end: usize,
    min_start: usize,
    max_end: usize,
}

/// The highlight item, the range is offset of the token in the tree.
#[derive(Debug, Default, Clone)]
struct HighlightItem {
    /// The byte range of the highlight in the text.
    range: Range<usize>,
    /// The highlight name, like `function`, `string`, `comment`, etc.
    name: SharedString,
}

impl HighlightItem {
    pub fn new(range: Range<usize>, name: impl Into<SharedString>) -> Self {
        Self {
            range,
            name: name.into(),
        }
    }
}

impl sum_tree::Item for HighlightItem {
    type Summary = HighlightSummary;
    fn summary(&self, _cx: &()) -> Self::Summary {
        HighlightSummary {
            count: 1,
            start: self.range.start,
            end: self.range.end,
            min_start: self.range.start,
            max_end: self.range.end,
        }
    }
}

impl sum_tree::Summary for HighlightSummary {
    type Context<'a> = &'a ();
    fn zero(_: Self::Context<'_>) -> Self {
        HighlightSummary {
            count: 0,
            start: usize::MIN,
            end: usize::MAX,
            min_start: usize::MAX,
            max_end: usize::MIN,
        }
    }

    fn add_summary(&mut self, other: &Self, _: Self::Context<'_>) {
        self.min_start = self.min_start.min(other.min_start);
        self.max_end = self.max_end.max(other.max_end);
        self.start = other.start;
        self.end = other.end;
        self.count += other.count;
    }
}

impl<'a> sum_tree::Dimension<'a, HighlightSummary> for usize {
    fn zero(_: &()) -> Self {
        0
    }

    fn add_summary(&mut self, _: &'a HighlightSummary, _: &()) {}
}

impl<'a> sum_tree::Dimension<'a, HighlightSummary> for Range<usize> {
    fn zero(_: &()) -> Self {
        Default::default()
    }

    fn add_summary(&mut self, summary: &'a HighlightSummary, _: &()) {
        self.start = summary.start;
        self.end = summary.end;
    }
}

impl SyntaxHighlighter {
    /// Create a new SyntaxHighlighter for the given language.
    pub fn new(lang: &str) -> Self {
        match Self::build_for_language(&lang) {
            Ok(result) => result,
            Err(err) => {
                tracing::warn!(
                    "SyntaxHighlighter init failed, fallback to use `text`, {}",
                    err
                );
                Self::build_for_language("text").unwrap()
            }
        }
    }

    /// Build an inert highlighter that never parses and creates no styles,
    /// for languages without a grammar.
    fn build_inert(language: SharedString) -> Self {
        Self {
            language,
            query: None,
            injections_query: None,
            locals_pattern_index: 0,
            highlights_pattern_index: 0,
            non_local_variable_patterns: Vec::new(),
            injection_content_capture_index: None,
            injection_language_capture_index: None,
            local_scope_capture_index: None,
            local_def_capture_index: None,
            local_def_value_capture_index: None,
            local_ref_capture_index: None,
            text: Rope::new(),
            parser: Parser::new(),
            tree: None,
            injection_layers: Vec::new(),
            combined_ranges: Vec::new(),
            injections_capped: false,
            injections_edited: false,
            injection_budget: Some(INJECTION_PARSE_TIMEOUT),
            moved_since_parse: None,
            injections_current: false,
        }
    }

    /// Build the highlighter for the given language.
    ///
    /// https://github.com/tree-sitter/tree-sitter/blob/v0.26.8/crates/highlight/src/highlight.rs#L339
    fn build_for_language(lang: &str) -> Result<Self> {
        let Some(config) = LanguageRegistry::singleton().language(&lang) else {
            return Err(anyhow!(
                "language {:?} is not registered in `LanguageRegistry`",
                lang
            ));
        };

        // Languages without a parser (neither a statically linked grammar nor a
        // registered parser factory) default to a highlighter that never parses
        // and creates no styles.
        if !LanguageRegistry::singleton().has_parser(lang) {
            return Ok(Self::build_inert(config.name.clone()));
        }

        let (mut parser, grammar) = LanguageRegistry::singleton().parser(lang)?;
        parser
            .set_language(&grammar)
            .context("parse set_language")?;
        let compiled = Compiled::for_language(&config, &grammar)?;
        Ok(Self {
            language: config.name.clone(),
            query: Some(compiled.query.clone()),
            injections_query: compiled.injections_query.clone(),
            locals_pattern_index: compiled.locals_pattern_index,
            highlights_pattern_index: compiled.highlights_pattern_index,
            non_local_variable_patterns: compiled.non_local_variable_patterns.clone(),
            injection_content_capture_index: compiled.injection_content_capture_index,
            injection_language_capture_index: compiled.injection_language_capture_index,
            local_scope_capture_index: compiled.local_scope_capture_index,
            local_def_capture_index: compiled.local_def_capture_index,
            local_def_value_capture_index: compiled.local_def_value_capture_index,
            local_ref_capture_index: compiled.local_ref_capture_index,
            text: Rope::new(),
            parser,
            tree: None,
            injection_layers: Vec::new(),
            combined_ranges: Vec::new(),
            injections_capped: false,
            injections_edited: false,
            injection_budget: Some(INJECTION_PARSE_TIMEOUT),
            moved_since_parse: None,
            injections_current: false,
        })
    }
}

/// atelier patch 3: a language's queries, compiled once for the whole process. Compiling Rust's takes
/// about 50 ms, and every new highlighter needed them: each editor that opened, and each `set_value`,
/// which drops the editor's highlighter. The queries never change after they are built, so every
/// highlighter of the language shares them, on any thread.
struct Compiled {
    query: Arc<Query>,
    injections_query: Option<Arc<Query>>,
    locals_pattern_index: usize,
    highlights_pattern_index: usize,
    non_local_variable_patterns: Vec<bool>,
    injection_content_capture_index: Option<u32>,
    injection_language_capture_index: Option<u32>,
    local_scope_capture_index: Option<u32>,
    local_def_capture_index: Option<u32>,
    local_def_value_capture_index: Option<u32>,
    local_ref_capture_index: Option<u32>,
}

/// By the language's name and a hash of its query sources, so a language registered again with new
/// queries compiles them anew.
static COMPILED: std::sync::LazyLock<std::sync::Mutex<HashMap<(SharedString, u64), Arc<Compiled>>>> =
    std::sync::LazyLock::new(Default::default);

impl Compiled {
    /// The queries of `lang`, compiled the first time any highlighter asks. The lock is held while
    /// they compile, so two threads that want the same language wait for one build.
    fn for_language(
        config: &GrammarConfig,
        grammar: &tree_sitter::Language,
    ) -> Result<Arc<Self>> {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        (&config.injections, &config.locals, &config.highlights).hash(&mut hasher);
        let key = (config.name.clone(), hasher.finish());
        let mut compiled = COMPILED.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(found) = compiled.get(&key) {
            return Ok(found.clone());
        }
        let built = Arc::new(Self::build(config, grammar)?);
        compiled.insert(key, built.clone());
        Ok(built)
    }

    fn build(config: &GrammarConfig, grammar: &tree_sitter::Language) -> Result<Self> {
        // Concatenate the query strings, keeping track of the start offset of each section.
        let mut query_source = String::new();
        query_source.push_str(&config.injections);
        let locals_query_offset = query_source.len();
        query_source.push_str(&config.locals);
        let highlights_query_offset = query_source.len();
        query_source.push_str(&config.highlights);

        // Construct a single query by concatenating the three query strings, but record the
        // range of pattern indices that belong to each individual string.
        let mut query = Query::new(grammar, &query_source).context("new query")?;

        let mut locals_pattern_index = 0;
        let mut highlights_pattern_index = 0;
        for i in 0..(query.pattern_count()) {
            let pattern_offset = query.start_byte_for_pattern(i);
            if pattern_offset < highlights_query_offset {
                if pattern_offset < highlights_query_offset {
                    highlights_pattern_index += 1;
                }
                if pattern_offset < locals_query_offset {
                    locals_pattern_index += 1;
                }
            }
        }

        let injections_query = if !config.injections.is_empty() {
            Query::new(grammar, &config.injections).ok().map(Arc::new)
        } else {
            None
        };

        // Injection layers are computed separately during parsing, so do not
        // emit injection captures from the main highlight query.
        for pattern_index in 0..locals_pattern_index {
            query.disable_pattern(pattern_index);
        }

        // Find all of the highlighting patterns that are disabled for nodes that
        // have been identified as local variables.
        let non_local_variable_patterns = (0..query.pattern_count())
            .map(|i| {
                query
                    .property_predicates(i)
                    .iter()
                    .any(|(prop, positive)| !*positive && prop.key.as_ref() == "local")
            })
            .collect();

        // Store the numeric ids for all of the special captures.
        let injection_content_capture_index = injections_query.as_ref().and_then(|q| {
            q.capture_names()
                .iter()
                .position(|name| *name == "injection.content")
                .map(|i| i as u32)
        });
        let injection_language_capture_index = injections_query.as_ref().and_then(|q| {
            q.capture_names()
                .iter()
                .position(|name| *name == "injection.language")
                .map(|i| i as u32)
        });
        let mut local_def_capture_index = None;
        let mut local_def_value_capture_index = None;
        let mut local_ref_capture_index = None;
        let mut local_scope_capture_index = None;
        for (i, name) in query.capture_names().iter().enumerate() {
            let i = Some(i as u32);
            match *name {
                "local.definition" => local_def_capture_index = i,
                "local.definition-value" => local_def_value_capture_index = i,
                "local.reference" => local_ref_capture_index = i,
                "local.scope" => local_scope_capture_index = i,
                _ => {}
            }
        }

        Ok(Self {
            query: Arc::new(query),
            injections_query,
            locals_pattern_index,
            highlights_pattern_index,
            non_local_variable_patterns,
            injection_content_capture_index,
            injection_language_capture_index,
            local_scope_capture_index,
            local_def_capture_index,
            local_def_value_capture_index,
            local_ref_capture_index,
        })
    }
}

impl SyntaxHighlighter {
    pub fn is_empty(&self) -> bool {
        self.text.len() == 0
    }

    /// Get the parsed tree (if available)
    pub fn tree(&self) -> Option<&Tree> {
        self.tree.as_ref()
    }

    /// Apply only the structural `edit` to the existing tree and update the stored text,
    /// without re-parsing. The injection layers move with it, so until a parse lands the old
    /// colours stand where their text went (see [`Self::background_parse`]).
    pub fn edit_tree(&mut self, edit: Option<InputEdit>, text: &Rope) {
        if let (Some(edit), Some(tree)) = (edit, self.tree.as_mut()) {
            tree.edit(&edit);
        }
        if let Some(edit) = edit {
            for layer in &mut self.injection_layers {
                layer.follow(&edit);
            }
            for (_, ranges) in &mut self.combined_ranges {
                for range in ranges.iter_mut() {
                    *range = follow_ts_range(range, &edit);
                }
            }
            // One span of new text for all the edits since the layers were whole: the earlier ones
            // moved by this one, then this one's.
            let wrote = edit.start_byte..edit.new_end_byte;
            self.moved_since_parse = match self.moved_since_parse.take() {
                Some(span) => {
                    let span = follow_range(&span, &edit);
                    Some(span.start.min(wrote.start)..span.end.max(wrote.end))
                }
                None if self.injections_current => Some(wrote),
                None => None,
            };
        } else {
            self.moved_since_parse = None;
        }
        self.text = text.clone();
        self.injections_current = false;
    }

    /// The text replaced whole: the tree and layers describe another text, so they go, and the
    /// rows draw plain until a parse lands.
    pub fn reset_tree(&mut self, text: &Rope) {
        self.tree = None;
        self.set_injection_layers(InjectionLayers::default());
        self.text = text.clone();
        self.injections_current = false;
    }

    /// What a parse on another thread needs: this highlighter's tree, as edited, its text and its
    /// injections. `None` for a language with no grammar.
    pub fn background_parse(&self) -> Option<BackgroundParse> {
        self.parser.language()?;
        let moved = self.moved_since_parse.clone().zip(self.injection_query_data()).map(|(span, data)| MovedLayers {
            data,
            layers: self.injection_layers.clone(),
            combined_ranges: self.combined_ranges.clone(),
            capped: self.injections_capped,
            span,
        });
        Some(BackgroundParse {
            language: self.language.clone(),
            old_tree: self.tree.clone(),
            text: self.text.clone(),
            injections: self.injection_parse_data(),
            moved,
        })
    }

    /// Takes a background parse when it was for the text held now, and says whether it did; one
    /// for an older text is dropped, and the moved colours stay until a newer one lands.
    pub fn apply_parsed(&mut self, parsed: ParsedTree) -> bool {
        if !self.text.eq(&parsed.text) {
            return false;
        }
        self.tree = Some(parsed.tree);
        self.set_injection_layers(parsed.injections);
        true
    }

    /// How long one injection layer's parse may take: 20ms by default, `None` for as long as it
    /// needs. For tests, which must not depend on the machine's speed.
    #[doc(hidden)]
    pub fn set_injection_budget(&mut self, budget: Option<Duration>) {
        self.injection_budget = budget;
    }

    /// Whether the injection layers are whole for the text: false when a layer's parse ran out of
    /// time, or a cap dropped some; the next update then builds them all again.
    #[doc(hidden)]
    pub fn injections_complete(&self) -> bool {
        self.injections_current
    }

    /// Whether the last update changed the injection layers in place rather than rebuilding them.
    #[doc(hidden)]
    pub fn injections_edited(&self) -> bool {
        self.injections_edited
    }

    /// Each injection layer's language and ranges, in order: for tests that compare two highlighters.
    #[doc(hidden)]
    pub fn injection_layer_ranges(&self) -> Vec<(SharedString, Vec<tree_sitter::Range>, Range<usize>)> {
        self.injection_layers
            .iter()
            .map(|l| (l.language_name.clone(), l.ranges.clone(), l.byte_range.clone()))
            .collect()
    }

    /// Returns the language name for this highlighter.
    pub fn language(&self) -> &SharedString {
        &self.language
    }

    /// Returns a reference to the current text.
    pub fn text(&self) -> &Rope {
        &self.text
    }

    /// Highlight the given text, returning a map from byte ranges to highlight captures.
    ///
    /// Uses incremental parsing by `edit` to efficiently update the highlighter's state.
    /// When `timeout` is `Some`, aborts if parsing exceeds the given duration
    /// and returns `false`. On timeout the old tree is preserved so highlighting
    /// still works with stale data, but `self.text` is updated so that the
    /// caller can send the current text to a background parse.
    /// When `timeout` is `None`, parsing runs to completion and always returns `true`.
    pub fn update(
        &mut self,
        edit: Option<InputEdit>,
        text: &Rope,
        timeout: Option<Duration>,
    ) -> bool {
        if self.text.eq(text) {
            return true;
        }

        // If there's no grammar for the language, just update the text.
        if self.parser.language().is_none() {
            self.text = text.clone();
            return true;
        }

        // Without an edit the whole text is new, and so are its injections.
        let incremental = edit.is_some();
        let edit = edit.unwrap_or(InputEdit {
            start_byte: 0,
            old_end_byte: 0,
            new_end_byte: text.len(),
            start_position: Point::new(0, 0),
            old_end_position: Point::new(0, 0),
            new_end_position: Point::new(0, 0),
        });

        let mut old_tree = self
            .tree
            .take()
            .unwrap_or(self.parser.parse("", None).unwrap());
        old_tree.edit(&edit);

        let mut timed_out = false;
        let start = Instant::now();
        let mut progress = |_: &tree_sitter::ParseState| -> ControlFlow<()> {
            let Some(budget) = timeout else {
                return ControlFlow::Continue(());
            };

            if start.elapsed() > budget {
                timed_out = true;
                return ControlFlow::Break(()); // Cancel execution
            }

            ControlFlow::Continue(())
        };

        let options = ParseOptions::new().progress_callback(&mut progress);
        let new_tree = self.parser.parse_with_options(
            &mut move |offset, _| {
                if offset >= text.len() {
                    ""
                } else {
                    let (chunk, chunk_byte_ix) = text.chunk(offset);
                    &chunk[offset - chunk_byte_ix..]
                }
            },
            Some(&old_tree),
            Some(options),
        );

        if timed_out || new_tree.is_none() {
            // Restore the old tree so highlighting continues with stale data.
            self.tree = Some(old_tree);
            self.text = text.clone();
            self.injections_current = false;
            return false;
        }

        let new_tree = new_tree.unwrap();
        self.text = text.clone();
        let edited = incremental
            && self.injections_current
            && self.edit_injection_layers(&edit, &old_tree, &new_tree);
        self.injections_edited = edited;
        if !edited {
            self.parse_injection_layers(&new_tree);
        }
        self.tree = Some(new_tree);
        true
    }

    /// Returns the data needed to compute injection layers on a background thread.
    /// Returns `None` if this language has no injections.
    pub(crate) fn injection_parse_data(&self) -> Option<InjectionParseData> {
        let query = self.injections_query.clone()?;
        Some(InjectionParseData {
            budget: self.injection_budget,
            query,
            content_capture_index: self.injection_content_capture_index,
            language_capture_index: self.injection_language_capture_index,
            old_layers: self
                .injection_layers
                .iter()
                .filter(|layer| !(layer.combined && layer.followed))
                .map(|layer| ReusableInjectionLayer {
                    language_name: layer.language_name.clone(),
                    highlight_query: layer.highlight_query.clone(),
                    ranges: layer.ranges.clone(),
                    tree: layer.tree.clone(),
                })
                .collect(),
        })
    }

    /// Compute injection layers from a freshly-parsed main tree.
    /// This is pure computation with no side effects and is safe to run on a
    /// background thread.
    pub(crate) fn compute_injection_layers(
        data: InjectionParseData,
        tree: &Tree,
        text: &Rope,
    ) -> InjectionLayers {
        let old_layer_trees: HashMap<_, _> = data
            .old_layers
            .iter()
            .map(|layer| {
                (
                    (layer.language_name.clone(), ranges_cache_key(&layer.ranges)),
                    &layer.tree,
                )
            })
            .collect();
        // Query objects are relatively expensive. Reuse one Arc per language
        // from the previous parse and compile only languages present in this
        // document, rather than eagerly retaining every registered grammar.
        let mut highlight_queries: HashMap<SharedString, Arc<Query>> = data
            .old_layers
            .iter()
            .map(|layer| (layer.language_name.clone(), layer.highlight_query.clone()))
            .collect();
        let mut resolved_languages = HashMap::new();
        let found = find_injections(
            &data,
            tree,
            text,
            None,
            &mut highlight_queries,
            &mut resolved_languages,
        );

        let mut combined_ranges: HashMap<SharedString, CombinedRanges> = HashMap::new();
        let mut new_layers = Vec::new();
        let mut non_combined_parses = 0usize;
        // A layer that ran out of time is missing; the next edit then builds them all again.
        let mut timed_out = false;
        let mut capped = false;
        for injection in found {
            if injection.combined {
                combined_ranges
                    .entry(injection.language_name.clone())
                    .or_insert_with(|| CombinedRanges {
                        ranges: Vec::new(),
                        byte_count: 0,
                        truncated: false,
                    })
                    .push_limited(injection.ranges);
                continue;
            }
            // Skip rather than break, so later combined ranges are still collected.
            if non_combined_parses >= MAX_NON_COMBINED_INJECTION_PARSES {
                capped = true;
                continue;
            }
            if !injection_ranges_within_limits(&injection.ranges) {
                continue;
            }

            non_combined_parses += 1;
            let old_tree = old_layer_trees
                .get(&(injection.language_name.clone(), ranges_cache_key(&injection.ranges)))
                .copied();
            if let Some(layer) = Self::parse_injection_layer(
                &injection.language_name,
                injection.highlight_query,
                injection.ranges,
                old_tree,
                text,
                false,
                data.budget,
                &mut timed_out,
            ) {
                new_layers.push(InjectionLayer {
                    match_range: injection.match_range,
                    ..layer
                });
            }
        }

        let mut complete = true;
        let mut raw_ranges = Vec::new();
        for (language_name, combined) in combined_ranges {
            complete &= !combined.truncated;
            let mut ranges = combined.ranges;
            if ranges.is_empty() {
                continue;
            }
            sort_ranges(&mut ranges);
            raw_ranges.push((language_name.clone(), ranges.clone()));
            let normalized = normalize_combined_injection_ranges(&language_name, ranges.clone());
            complete &= normalized.last() == ranges.last();
            if normalized.is_empty() {
                continue;
            }
            let old_tree = old_layer_trees
                .get(&(language_name.clone(), ranges_cache_key(&normalized)))
                .copied();
            let Some(highlight_query) = highlight_queries.get(&language_name).cloned() else {
                continue;
            };
            if let Some(layer) = Self::parse_injection_layer(
                &language_name,
                highlight_query,
                normalized,
                old_tree,
                text,
                true,
                data.budget,
                &mut timed_out,
            ) {
                new_layers.push(layer);
            }
        }
        raw_ranges.sort_by(|a, b| a.0.as_ref().cmp(b.0.as_ref()));
        sort_layers(&mut new_layers);
        InjectionLayers {
            layers: new_layers,
            combined_ranges: raw_ranges,
            capped,
            complete: complete && !timed_out,
        }
    }

    /// Update the injection layers for `edit` in place (see [`update_layers`]). Returns `false`,
    /// changing nothing, when only a full pass can be exact.
    fn edit_injection_layers(&mut self, edit: &InputEdit, old_tree: &Tree, new_tree: &Tree) -> bool {
        let Some(data) = self.injection_query_data() else {
            return false;
        };
        let moved = Moved::By(*edit);
        let updated = update_layers(&data, &self.injection_layers, &self.combined_ranges, self.injections_capped, &moved, old_tree, new_tree, &self.text);
        match updated {
            Some(layers) => {
                self.set_injection_layers(layers);
                true
            }
            None => false,
        }
    }

    /// The injections query and its settings, with no old layers.
    fn injection_query_data(&self) -> Option<InjectionParseData> {
        Some(InjectionParseData {
            budget: self.injection_budget,
            query: self.injections_query.clone()?,
            content_capture_index: self.injection_content_capture_index,
            language_capture_index: self.injection_language_capture_index,
            old_layers: Vec::new(),
        })
    }

    /// Parse one injection layer over the given included ranges.
    /// Reuses the previous tree only when the language and byte ranges still match.
    fn parse_injection_layer(
        language_name: &SharedString,
        highlight_query: Arc<Query>,
        ranges: Vec<tree_sitter::Range>,
        old_tree: Option<&Tree>,
        text: &Rope,
        combined: bool,
        budget: Option<Duration>,
        timed_out_any: &mut bool,
    ) -> Option<InjectionLayer> {
        let (mut parser, grammar) = LanguageRegistry::singleton().parser(language_name).ok()?;
        parser.set_language(&grammar).ok()?;
        parser.set_included_ranges(&ranges).ok()?;
        let parse_start = Instant::now();
        let mut timed_out = false;
        let mut progress = |_: &tree_sitter::ParseState| -> ControlFlow<()> {
            if budget.is_some_and(|budget| parse_start.elapsed() >= budget) {
                timed_out = true;
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        };
        let options = ParseOptions::new().progress_callback(&mut progress);

        let new_tree = parser.parse_with_options(
            &mut |offset, _| {
                if offset >= text.len() {
                    ""
                } else {
                    let (chunk, chunk_byte_ix) = text.chunk(offset);
                    &chunk[offset - chunk_byte_ix..]
                }
            },
            old_tree,
            Some(options),
        );
        // A parse stopped by the budget returns nothing, so the flag is read before the result.
        if timed_out {
            *timed_out_any = true;
            return None;
        }
        let new_tree = new_tree?;

        let byte_range = bounding_byte_range(&ranges)?;
        Some(InjectionLayer {
            language_name: language_name.clone(),
            highlight_query,
            ranges,
            match_range: byte_range.clone(),
            byte_range,
            tree: new_tree,
            combined,
            followed: false,
        })
    }

    fn set_injection_layers(&mut self, injections: InjectionLayers) {
        self.moved_since_parse = None;
        self.injection_layers = injections.layers;
        self.combined_ranges = injections.combined_ranges;
        self.injections_capped = injections.capped;
        self.injections_current = injections.complete;
    }

    /// Parse injection layers after the main tree is updated.
    /// pattern: parse once in update, query many times in render.
    fn parse_injection_layers(&mut self, tree: &Tree) {
        let Some(data) = self.injection_parse_data() else {
            self.set_injection_layers(InjectionLayers::default());
            return;
        };
        let injections = Self::compute_injection_layers(data, tree, &self.text.clone());
        self.set_injection_layers(injections);
    }

    /// Match the visible ranges of nodes in the Tree for highlighting.
    fn match_styles(&self, range: Range<usize>) -> Vec<HighlightItem> {
        let mut highlights = vec![];
        let mut injection_highlights = vec![];
        let Some(tree) = &self.tree else {
            return highlights;
        };

        let Some(query) = &self.query else {
            return highlights;
        };

        let root_node = tree.root_node();
        let source = &self.text;

        // Query pre-parsed injection layers.
        let mut last_layer_start = 0;
        for layer in &self.injection_layers {
            debug_assert!(layer.byte_range.start >= last_layer_start);
            last_layer_start = layer.byte_range.start;

            if layer.byte_range.end <= range.start {
                continue;
            }

            // Layers are sorted by start byte in compute_injection_layers.
            if layer.byte_range.start >= range.end {
                break;
            }

            let query = &layer.highlight_query;

            let mut query_cursor = QueryCursor::new();
            query_cursor.set_byte_range(range.clone());

            let mut matches =
                query_cursor.matches(query, layer.tree.root_node(), TextProvider(&self.text));

            let mut last_end = 0usize;
            while let Some(m) = matches.next() {
                let allow_overlapping_captures = query
                    .property_settings(m.pattern_index)
                    .iter()
                    .any(|prop| prop.key.as_ref() == "highlight.allow-overlap");

                for cap in m.captures {
                    let node_range = cap.node.start_byte()..cap.node.end_byte();

                    if !allow_overlapping_captures && node_range.start < last_end {
                        continue;
                    }

                    if let Some(highlight_name) = query.capture_names().get(cap.index as usize) {
                        if !allow_overlapping_captures {
                            last_end = node_range.end;
                        }
                        injection_highlights.push(HighlightItem::new(
                            node_range,
                            SharedString::from(highlight_name.to_string()),
                        ));
                    }
                }
            }
        }

        let mut query_cursor = QueryCursor::new();
        query_cursor.set_byte_range(range.clone());

        let mut matches = query_cursor.matches(query, root_node, TextProvider(source));

        while let Some(query_match) = matches.next() {
            for cap in query_match.captures {
                let node = cap.node;

                let Some(highlight_name) = query.capture_names().get(cap.index as usize) else {
                    continue;
                };

                let node_range: Range<usize> = node.start_byte()..node.end_byte();
                let highlight_name = SharedString::from(highlight_name.to_string());

                // Merge near range and same highlight name
                let last_item = highlights.last();
                let last_range = last_item.map(|item| &item.range).unwrap_or(&(0..0));
                let last_highlight_name = last_item.map(|item| item.name.clone());

                if last_range == &node_range {
                    // case:
                    // last_range: 213..220, last_highlight_name: Some("property")
                    // last_range: 213..220, last_highlight_name: Some("string")
                    highlights.push(HighlightItem::new(
                        node_range,
                        last_highlight_name.unwrap_or(highlight_name),
                    ));
                } else {
                    highlights.push(HighlightItem::new(node_range, highlight_name.clone()));
                }
            }
        }

        // Injected languages are more specific than the host language. Keep
        // them last so their colors win over broad Markdown captures such as
        // `fenced_code_block @text.literal`.
        highlights.extend(injection_highlights);

        // DO NOT REMOVE THIS PRINT, it's useful for debugging
        // for item in highlights {
        //     println!("item: {:?}", item);
        // }

        highlights
    }

    /// Returns the syntax highlight styles for a range of text.
    ///
    /// The argument `range` is the range of bytes in the text to highlight.
    ///
    /// Returns a vector of tuples where each tuple contains:
    /// - A byte range relative to the text
    /// - The corresponding highlight style for that range
    ///
    /// # Example
    ///
    /// ```no_run
    /// # mod gpui_kit { pub extern crate gpui_component as component; }
    /// use gpui_kit::component::highlighter::{HighlightTheme, SyntaxHighlighter};
    /// use ropey::Rope;
    ///
    /// let code = "fn main() {\n    println!(\"Hello\");\n}";
    /// let rope = Rope::from_str(code);
    /// let mut highlighter = SyntaxHighlighter::new("rust");
    /// highlighter.update(None, &rope, None);
    ///
    /// let theme = HighlightTheme::default_dark();
    /// let range = 0..code.len();
    /// let styles = highlighter.styles(&range, &*theme);
    /// ```
    pub fn styles(
        &self,
        range: &Range<usize>,
        theme: &dyn gpui_base::input::HighlightStyleResolver,
    ) -> Vec<(Range<usize>, HighlightStyle)> {
        let mut styles = vec![];
        let start_offset = range.start;

        let highlights = self.match_styles(range.clone());

        // let mut iter_count = 0;
        for item in highlights {
            // iter_count += 1;
            let node_range = &item.range;
            let name = &item.name;

            // Avoid start larger than end
            let mut node_range = node_range.start.max(range.start)..node_range.end.min(range.end);
            if node_range.start > node_range.end {
                node_range.end = node_range.start;
            }
            // The tree can be stale while a background reparse is pending
            // (sync-parse timeout, or the large-text `edit_tree` path), so
            // node offsets may fall inside multi-byte characters of the
            // current text. Snap to char boundaries — text shaping panics on
            // a mid-char style boundary.
            node_range = self.text.clip_offset(node_range.start, Bias::Left)
                ..self.text.clip_offset(node_range.end, Bias::Right);
            if node_range.is_empty() {
                continue;
            }

            styles.push((node_range, theme.style(name.as_ref()).unwrap_or_default()));
        }

        // If the matched styles is empty, return a default range.
        if styles.len() == 0 {
            return vec![(start_offset..range.end, HighlightStyle::default())];
        }

        let styles = unique_styles(&range, styles);

        // NOTE: DO NOT remove this comment, it is used for debugging.
        // for style in &styles {
        //     println!("---- style: {:?} - {:?}", style.0, style.1.color);
        // }
        // println!("--------------------------------");

        styles
    }
}

/// To merge intersection ranges, let the subsequent range cover
/// the previous overlapping range and split the previous range.
///
/// From:
///
/// AA
///   BBB
///    CCCCC
///      DD
///         EEEE
///
/// To:
///
/// AABCCDDCEEEE
pub(crate) fn unique_styles(
    total_range: &Range<usize>,
    styles: Vec<(Range<usize>, HighlightStyle)>,
) -> Vec<(Range<usize>, HighlightStyle)> {
    let styles: Vec<_> = styles
        .into_iter()
        .filter(|(range, _)| !range.is_empty())
        .collect();

    if styles.is_empty() {
        return styles;
    }

    // Create intervals: (position, is_start, style_index)
    let mut intervals: Vec<(usize, bool, usize)> = Vec::with_capacity(styles.len() * 2 + 2);
    for (i, (range, _)) in styles.iter().enumerate() {
        intervals.push((range.start, true, i));
        intervals.push((range.end, false, i));
    }

    intervals.push((total_range.start, true, usize::MAX));
    intervals.push((total_range.end, false, usize::MAX));

    // Sort by position, with ends before starts at same position
    // This ensures we close ranges before opening new ones at the same position
    intervals.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

    // Track significant intervals (where style ranges end) for merging decisions
    let mut significant_intervals: BTreeSet<usize> = BTreeSet::new();
    for (range, _) in &styles {
        significant_intervals.insert(range.end);
    }

    let mut result: Vec<(Range<usize>, HighlightStyle)> = Vec::new();
    let mut active_styles: Vec<usize> = Vec::new();
    let mut last_pos = total_range.start;

    for (pos, is_start, style_idx) in intervals {
        // Skip total_range boundaries in active set management
        let is_boundary = style_idx == usize::MAX;

        if pos > last_pos {
            let interval = last_pos..pos;
            let combined_style = if active_styles.is_empty() {
                HighlightStyle::default()
            } else {
                let mut combined = HighlightStyle::default();
                for &idx in &active_styles {
                    merge_highlight_style(&mut combined, &styles[idx].1);
                }
                combined
            };
            result.push((interval, combined_style));
        }

        if !is_boundary {
            if is_start {
                active_styles.push(style_idx);
            } else {
                active_styles.retain(|&i| i != style_idx);
            }
        }

        last_pos = pos;
    }

    // Merge adjacent ranges with the same style, but not across significant boundaries
    let mut merged: Vec<(Range<usize>, HighlightStyle)> = Vec::with_capacity(result.len());
    for (range, style) in result {
        if let Some((last_range, last_style)) = merged.last_mut() {
            if last_range.end == range.start
                && *last_style == style
                && !significant_intervals.contains(&range.start)
            {
                // Merge adjacent ranges with same style, but not across significant boundaries
                last_range.end = range.end;
                continue;
            }
        }
        merged.push((range, style));
    }

    merged
}

/// Merge other style (Other on top)
fn merge_highlight_style(style: &mut HighlightStyle, other: &HighlightStyle) {
    if let Some(color) = other.color {
        style.color = Some(color);
    }
    if let Some(font_weight) = other.font_weight {
        style.font_weight = Some(font_weight);
    }
    if let Some(font_style) = other.font_style {
        style.font_style = Some(font_style);
    }
    if let Some(background_color) = other.background_color {
        style.background_color = Some(background_color);
    }
    if let Some(underline) = other.underline {
        style.underline = Some(underline);
    }
    if let Some(strikethrough) = other.strikethrough {
        style.strikethrough = Some(strikethrough);
    }
    if let Some(fade_out) = other.fade_out {
        style.fade_out = Some(fade_out);
    }
}

#[cfg(test)]
mod tests {
    use gpui::Hsla;

    use super::*;
    use crate::Colorize as _;

    fn color_style(color: Hsla) -> HighlightStyle {
        let mut style = HighlightStyle::default();
        style.color = Some(color);
        style
    }

    #[test]
    fn test_plain_text_never_parses() {
        // "text" has no grammar, the highlighter shouldn't parse.
        let mut highlighter = SyntaxHighlighter::new("text");
        let rope = Rope::from("hello {\"a\": 1}\nworld");
        assert!(highlighter.update(None, &rope, None));
        assert!(highlighter.tree().is_none());
        assert_eq!(highlighter.text().to_string(), rope.to_string());

        let theme = HighlightTheme::default_dark();
        let styles = highlighter.styles(&(0..rope.len()), theme.as_ref());
        assert_eq!(styles, vec![(0..rope.len(), HighlightStyle::default())]);

        // Unregistered languages fall back to plain text.
        let mut highlighter = SyntaxHighlighter::new("no-such-language");
        assert!(highlighter.update(None, &rope, None));
        assert!(highlighter.tree().is_none());
    }

    /// While a background reparse is pending (sync-parse timeout, or the
    /// large-text `edit_tree` path), `styles()` serves ranges from a stale
    /// tree. Those must still land on char boundaries of the current text,
    /// or text shaping panics on multi-byte characters.
    #[cfg(feature = "tree-sitter-languages")]
    #[test]
    fn test_stale_tree_styles_snap_to_char_boundaries() {
        let mut highlighter = SyntaxHighlighter::new("markdown");
        let old = Rope::from("# hello world\n*emphasis* and `code` here\n");
        assert!(highlighter.update(None, &old, None));
        assert!(highlighter.tree().is_some());

        // Swap the text without reparsing: the tree is now stale and its node
        // offsets point into the middle of the new text's CJK characters.
        let new = Rope::from("# 你好，世界\n你好，*世界* 与 `代码`\n");
        highlighter.edit_tree(None, &new);

        let theme = HighlightTheme::default_dark();
        let styles = highlighter.styles(&(0..new.len()), theme.as_ref());
        assert!(!styles.is_empty());
        for (range, _) in &styles {
            assert!(
                new.is_char_boundary(range.start) && new.is_char_boundary(range.end),
                "style range {range:?} is not on char boundaries of the current text"
            );
        }
    }

    #[cfg(feature = "tree-sitter-languages")]
    fn has_highlight_covering(
        highlights: &[HighlightItem],
        source: &str,
        text: &str,
        highlight_name: &str,
    ) -> bool {
        let start = source.find(text).expect("text should exist in source");
        let end = start + text.len();
        highlights.iter().any(|item| {
            item.name.as_ref() == highlight_name
                && item.range.start <= start
                && item.range.end >= end
        })
    }

    #[track_caller]
    fn assert_unique_styles(
        range: Range<usize>,
        left: Vec<(Range<usize>, HighlightStyle)>,
        right: Vec<(Range<usize>, HighlightStyle)>,
    ) {
        fn color_name(c: Option<Hsla>) -> String {
            match c {
                Some(c) => {
                    if c == gpui::red() {
                        "red".to_string()
                    } else if c == gpui::green() {
                        "green".to_string()
                    } else if c == gpui::blue() {
                        "blue".to_string()
                    } else {
                        c.to_hex()
                    }
                }
                None => "clean".to_string(),
            }
        }

        let left = unique_styles(&range, left);
        if left.len() != right.len() {
            println!("\n---------------------------------------------");
            for (range, style) in left.iter() {
                println!("({:?}, {})", range, color_name(style.color));
            }
            println!("---------------------------------------------");
            panic!("left {} styles, right {} styles", left.len(), right.len());
        }
        for (left, right) in left.into_iter().zip(right) {
            if left.1.color != right.1.color || left.0 != right.0 {
                panic!(
                    "\n left: ({:?}, {})\nright: ({:?}, {})\n",
                    left.0,
                    color_name(left.1.color),
                    right.0,
                    color_name(right.1.color)
                );
            }
        }
    }

    #[test]
    #[cfg(feature = "tree-sitter-languages")]
    fn test_html_style_injects_css_highlights() {
        let html = r#"<style>
.card { color: #336699; }
</style>
"#;

        let rope = Rope::from_str(html);
        let mut highlighter = SyntaxHighlighter::new("html");
        highlighter.update(None, &rope, None);

        let highlights = highlighter.match_styles(0..html.len());

        assert!(
            has_highlight_covering(&highlights, html, "color", "property"),
            "CSS property names inside style elements should be highlighted"
        );
        assert!(
            has_highlight_covering(&highlights, html, "#336699", "string.special"),
            "CSS color values inside style elements should be highlighted"
        );
    }

    #[test]
    #[cfg(feature = "tree-sitter-languages")]
    fn test_html_script_injects_javascript_highlights() {
        let html = r#"<script>
const answer = 42;
console.log(answer);
</script>
"#;

        let rope = Rope::from_str(html);
        let mut highlighter = SyntaxHighlighter::new("html");
        highlighter.update(None, &rope, None);

        let highlights = highlighter.match_styles(0..html.len());

        assert!(
            has_highlight_covering(&highlights, html, "const", "keyword"),
            "JavaScript keywords inside script elements should be highlighted"
        );
        assert!(
            has_highlight_covering(&highlights, html, "answer", "variable"),
            "JavaScript identifiers inside script elements should be highlighted"
        );
    }

    #[test]
    #[cfg(feature = "tree-sitter-languages")]
    fn test_markdown_fenced_code_injects_captured_language() {
        let markdown = "```rs\nfn first() {}\n```\n\n```rust\nfn second() {}\n```\n";
        let rope = Rope::from_str(markdown);
        let mut highlighter = SyntaxHighlighter::new("markdown");

        assert!(highlighter.update(None, &rope, None));

        let rust_layers = highlighter
            .injection_layers
            .iter()
            .filter(|layer| layer.language_name.as_ref() == "rust")
            .collect::<Vec<_>>();
        assert_eq!(rust_layers.len(), 2);
        assert!(
            Arc::ptr_eq(
                &rust_layers[0].highlight_query,
                &rust_layers[1].highlight_query
            ),
            "fences using the same canonical language should share one highlight query"
        );

        let highlights = highlighter.match_styles(0..markdown.len());
        for function in ["first", "second"] {
            assert!(
                has_highlight_covering(&highlights, markdown, function, "function"),
                "Rust function {function:?} should be highlighted inside its fence"
            );
        }

        let theme = HighlightTheme::default_dark();
        let styles = highlighter.styles(&(0..markdown.len()), theme.as_ref());
        let keyword_start = markdown.find("fn first").unwrap();
        let keyword_color = theme.style("keyword").and_then(|style| style.color);
        assert!(styles.iter().any(|(range, style)| {
            range.start <= keyword_start
                && range.end >= keyword_start + 2
                && style.color == keyword_color
        }));
    }

    #[test]
    #[cfg(feature = "tree-sitter-languages")]
    fn test_markdown_unknown_fence_language_does_not_allocate_layer() {
        let markdown = "```not-a-registered-language\nplain content\n```\n";
        let rope = Rope::from_str(markdown);
        let mut highlighter = SyntaxHighlighter::new("markdown");

        assert!(highlighter.update(None, &rope, None));
        assert!(
            highlighter
                .injection_layers
                .iter()
                .all(|layer| { layer.language_name.as_ref() != "not-a-registered-language" })
        );
    }

    #[test]
    #[cfg(feature = "tree-sitter-languages")]
    fn test_markdown_fenced_code_highlights_blocks_beyond_previous_limit() {
        const FENCE_COUNT: usize = 384;
        const _: () = assert!(FENCE_COUNT <= MAX_NON_COMBINED_INJECTION_PARSES);
        let markdown = (0..FENCE_COUNT)
            .map(|i| format!("```rust\nfn function_{i}() {{}}\n```\n"))
            .collect::<String>();
        let rope = Rope::from_str(&markdown);
        let mut highlighter = SyntaxHighlighter::new("markdown");

        assert!(highlighter.update(None, &rope, None));
        assert_eq!(highlighter.injection_layers.len(), FENCE_COUNT);
        assert!(
            highlighter
                .injection_layers
                .iter()
                .all(|layer| layer.language_name.as_ref() == "rust")
        );

        let highlights = highlighter.match_styles(0..markdown.len());
        assert!(has_highlight_covering(
            &highlights,
            &markdown,
            "function_383",
            "function"
        ));
    }

    #[test]
    #[cfg(feature = "tree-sitter-languages")]
    fn test_markdown_fenced_code_injection_layers_are_bounded() {
        const FENCE_COUNT: usize = MAX_NON_COMBINED_INJECTION_PARSES + 128;
        // The paragraph trails the fences so the combined match is only reached
        // once the non-combined budget is already exhausted.
        let markdown = format!(
            "{}\nparagraph *inline*\n",
            (0..FENCE_COUNT)
                .map(|i| format!("```rust\nfn function_{i}() {{}}\n```\n"))
                .collect::<String>()
        );
        let rope = Rope::from_str(&markdown);
        let mut highlighter = SyntaxHighlighter::new("markdown");

        assert!(highlighter.update(None, &rope, None));
        assert_eq!(
            highlighter
                .injection_layers
                .iter()
                .filter(|layer| layer.language_name.as_ref() == "rust")
                .count(),
            MAX_NON_COMBINED_INJECTION_PARSES
        );
        assert!(
            highlighter
                .injection_layers
                .iter()
                .any(|layer| layer.language_name.as_ref() == "markdown_inline"),
            "the non-combined budget should not starve combined injection layers"
        );

        let highlights = highlighter.match_styles(0..markdown.len());
        assert!(has_highlight_covering(
            &highlights,
            &markdown,
            "function_0",
            "function"
        ));
        assert!(
            !has_highlight_covering(
                &highlights,
                &markdown,
                &format!("function_{}", FENCE_COUNT - 1),
                "function"
            ),
            "fences past the budget keep host highlighting but get no injected tokens"
        );
    }

    #[test]
    #[cfg(feature = "tree-sitter-languages")]
    fn test_php_combined_injection_closing_tags() {
        let php_code = r#"<?php
$x = 1;
?>
<html>
<body>
  <h1><?php echo "Hello"; ?></h1>
  <ul>
    <?php foreach ($items as $item): ?>
      <li><?php echo $item; ?></li>
    <?php endforeach; ?>
  </ul>
</body>
</html>
"#;

        let rope = Rope::from_str(php_code);
        let mut highlighter = SyntaxHighlighter::new("php");
        highlighter.update(None, &rope, None);

        let full_range = 0..php_code.len();
        let highlights = highlighter.match_styles(full_range);

        // Verify all closing HTML tags are highlighted
        let closing_tags = ["</h1>", "</li>", "</ul>", "</body>", "</html>"];
        for tag in closing_tags {
            let pos = php_code.find(tag).unwrap();
            let tag_name_start = pos + 2; // after "</"
            let tag_name_end = tag_name_start + tag.len() - 3; // before ">"

            let has_highlight = highlights
                .iter()
                .any(|item| item.range.start <= tag_name_start && item.range.end >= tag_name_end);

            assert!(
                has_highlight,
                "closing tag {} at byte {} should be highlighted",
                tag, pos
            );
        }
    }

    #[test]
    #[cfg(feature = "tree-sitter-languages")]
    fn test_markdown_inline_injection_layers_are_bounded() {
        let markdown = (0..(MAX_INJECTION_RANGES + 1024))
            .map(|i| format!("paragraph {i} *x*\n\n"))
            .collect::<String>();
        let rope = Rope::from_str(markdown.as_str());
        let mut highlighter = SyntaxHighlighter::new("markdown");

        assert!(highlighter.update(None, &rope, None));
        assert!(
            highlighter.injection_layers.len() <= 1,
            "markdown_inline should be combined instead of one layer per inline node"
        );

        if let Some(layer) = highlighter
            .injection_layers
            .iter()
            .find(|layer| layer.language_name.as_ref() == "markdown_inline")
        {
            assert!(layer.ranges.len() <= MAX_INJECTION_RANGES);
            assert!(injection_ranges_byte_count(&layer.ranges) <= MAX_INJECTION_BYTES);
        }

        let plain_markdown = (0..1024)
            .map(|i| format!("paragraph {i} plain\n\n"))
            .collect::<String>();
        let plain_rope = Rope::from_str(plain_markdown.as_str());
        let mut plain_highlighter = SyntaxHighlighter::new("markdown");

        assert!(plain_highlighter.update(None, &plain_rope, None));
        assert!(
            plain_highlighter
                .injection_layers
                .iter()
                .all(|layer| layer.language_name.as_ref() != "markdown_inline"),
            "plain inline ranges should not create markdown_inline injection layers"
        );
    }

    #[test]
    #[cfg(feature = "tree-sitter-languages")]
    fn test_markdown_inline_code_spans_in_list_items() {
        let markdown = "- `one`\n- `two`\n- `three`\n\nLater `four`\n";
        let rope = Rope::from_str(markdown);
        let mut highlighter = SyntaxHighlighter::new("markdown");
        highlighter.update(None, &rope, None);

        let highlights = highlighter.match_styles(0..markdown.len());
        for text in ["one", "two", "three", "four"] {
            assert!(
                has_highlight_covering(&highlights, markdown, text, "text.code.span"),
                "{text:?} should be highlighted as a code span"
            );
        }

        let prose_start = markdown.find("Later").unwrap();
        let prose_end = prose_start + "Later".len();
        assert!(
            !highlights.iter().any(|item| {
                item.name.as_ref() == "text.code.span"
                    && item.range.start <= prose_start
                    && item.range.end >= prose_end
            }),
            "plain prose after the list should not be highlighted as a code span"
        );
    }

    #[test]
    #[cfg(feature = "tree-sitter-languages")]
    fn test_highlight_allow_overlap_property_combines_nested_captures() {
        let markdown = "This has **_bold and italic_** and **bold _with_ italic** text.";
        let rope = Rope::from_str(markdown);
        let mut highlighter = SyntaxHighlighter::new("markdown");
        highlighter.update(None, &rope, None);

        let theme = HighlightTheme::default_dark();
        let styles = highlighter.styles(&(0..markdown.len()), theme.as_ref());
        for text in ["bold and italic", "with"] {
            let start = markdown.find(text).unwrap();
            let end = start + text.len();

            assert!(
                styles.iter().any(|(range, style)| {
                    range.start <= start
                        && range.end >= end
                        && style.font_weight == Some(gpui::FontWeight::BOLD)
                        && style.font_style == Some(gpui::FontStyle::Italic)
                }),
                "{text:?} should combine bold and italic styles"
            );
        }

        let highlights = highlighter.match_styles(0..markdown.len());
        let delimiter_start = markdown.find("_with_").unwrap();
        let delimiter_end = delimiter_start + "_".len();

        assert!(
            highlights.iter().any(|item| {
                item.name.as_ref() == "punctuation.delimiter"
                    && item.range.start <= delimiter_start
                    && item.range.end >= delimiter_end
            }),
            "overlap-enabled captures should not hide nested delimiter highlights"
        );
    }

    #[test]
    fn test_unique_styles() {
        let red = color_style(gpui::red());
        let green = color_style(gpui::green());
        let blue = color_style(gpui::blue());
        let clean = HighlightStyle::default();

        assert_unique_styles(
            0..65,
            vec![
                (2..10, clean),
                (2..10, clean),
                (5..11, red),
                (2..6, clean),
                (10..15, green),
                (15..30, clean),
                (29..35, blue),
                (35..40, green),
                (45..60, blue),
            ],
            vec![
                (0..5, clean),
                (5..6, red),
                (6..10, red),
                (10..11, green),
                (11..15, green),
                (15..29, clean),
                (29..30, blue),
                (30..35, blue),
                (35..40, green),
                (40..45, clean),
                (45..60, blue),
                (60..65, clean),
            ],
        );

        assert_unique_styles(
            0..10,
            vec![(2..2, red), (4..6, green)],
            vec![(0..4, clean), (4..6, green), (6..10, clean)],
        );
    }

}
