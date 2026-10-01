use std::{
    cell::{Cell, RefCell},
    ops::Range,
    rc::Rc,
    time::Duration,
};

use gpui::{HighlightStyle, SharedString, Task};
use gpui_base::input::{
    EditorState, FoldRange, HighlightStyleResolver, InputEdit as BaseInputEdit, InputHighlighter,
    InputHighlighterFactory,
};
use ropey::Rope;
use tree_sitter::{InputEdit, Point};

use super::{LanguageRegistry, ParseQueue, SyntaxHighlighter};

pub(crate) fn input_highlighter_factory() -> InputHighlighterFactory {
    Rc::new(|language| {
        LanguageRegistry::singleton().has_parser(language).then(|| {
            Box::new(TreeSitterInputHighlighter::new(language)) as Box<dyn InputHighlighter>
        })
    })
}

struct TreeSitterInputHighlighter {
    inner: Rc<RefCell<SyntaxHighlighter>>,
    parse_task: Rc<RefCell<Option<Task<()>>>>,
    queue: Rc<RefCell<ParseQueue>>,
    /// Whether the editor wants fold ranges, as its latest update said.
    folding: Rc<Cell<bool>>,
}

impl TreeSitterInputHighlighter {
    fn new(language: &str) -> Self {
        Self {
            inner: Rc::new(RefCell::new(SyntaxHighlighter::new(language))),
            parse_task: Rc::new(RefCell::new(None)),
            queue: Rc::default(),
            folding: Rc::default(),
        }
    }
}

impl SyntaxHighlighter {
    pub(crate) fn update_input(
        &mut self,
        edit: Option<BaseInputEdit>,
        text: &Rope,
        timeout: Option<Duration>,
    ) -> bool {
        self.update(edit.map(to_tree_sitter_edit), text, timeout)
    }
}

impl InputHighlighter for TreeSitterInputHighlighter {
    fn language(&self) -> SharedString {
        self.inner.borrow().language().clone()
    }

    /// atelier patch: the UI thread only applies the edit, which moves the old colours with the
    /// text. Every parse, whatever the text's size, runs on a background thread, one at a time;
    /// keystrokes during it coalesce into one next parse, and a result for an older text is dropped.
    fn update(
        &mut self,
        edit: Option<BaseInputEdit>,
        text: &Rope,
        folding: bool,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<EditorState>,
    ) {
        {
            let mut highlighter = self.inner.borrow_mut();
            match edit.map(to_tree_sitter_edit) {
                Some(edit) => highlighter.edit_tree(Some(edit), text),
                None => highlighter.reset_tree(text),
            }
        }
        self.folding.set(folding);
        if !self.queue.borrow_mut().request() {
            return;
        }

        let (highlighter, queue, folding) = (self.inner.clone(), self.queue.clone(), self.folding.clone());
        let task = cx.spawn_in(window, async move |entity, cx| {
            loop {
                let job = highlighter.borrow().background_parse();
                let fold = folding.get();
                let parsed = cx
                    .background_executor()
                    .spawn(async move {
                        let parsed = job?.run()?;
                        let folds = if fold { extract_fold_ranges(parsed.tree()) } else { Vec::new() };
                        Some((parsed, folds))
                    })
                    .await;
                if let Some((parsed, folds)) = parsed {
                    if highlighter.borrow_mut().apply_parsed(parsed) {
                        let _ = entity.update(cx, |state, cx| {
                            state.apply_highlighter_fold_candidates(folds, cx);
                            cx.notify();
                        });
                    }
                }
                if !queue.borrow_mut().finish() {
                    break;
                }
            }
        });
        self.parse_task.borrow_mut().replace(task);
    }

    fn styles(
        &self,
        range: &Range<usize>,
        resolver: &dyn HighlightStyleResolver,
    ) -> Vec<(Range<usize>, HighlightStyle)> {
        self.inner.borrow().styles(range, resolver)
    }
    fn fold_ranges(&self, _: &Rope) -> Vec<FoldRange> {
        self.inner
            .borrow()
            .tree()
            .map(extract_fold_ranges)
            .unwrap_or_default()
    }

    fn fold_ranges_for_edit(&self, range: Range<usize>, _: &Rope) -> Vec<FoldRange> {
        self.inner
            .borrow()
            .tree()
            .map(|tree| extract_fold_ranges_in_range(tree, range))
            .unwrap_or_default()
    }
}

fn to_tree_sitter_edit(edit: BaseInputEdit) -> InputEdit {
    InputEdit {
        start_byte: edit.start_byte,
        old_end_byte: edit.old_end_byte,
        new_end_byte: edit.new_end_byte,
        start_position: Point::new(edit.start_position.row, edit.start_position.column),
        old_end_position: Point::new(edit.old_end_position.row, edit.old_end_position.column),
        new_end_position: Point::new(edit.new_end_position.row, edit.new_end_position.column),
    }
}

fn extract_fold_ranges(tree: &tree_sitter::Tree) -> Vec<FoldRange> {
    extract_fold_ranges_in_range(tree, 0..usize::MAX)
}

fn extract_fold_ranges_in_range(
    tree: &tree_sitter::Tree,
    byte_range: Range<usize>,
) -> Vec<FoldRange> {
    fn collect(node: tree_sitter::Node, bytes: &Range<usize>, ranges: &mut Vec<FoldRange>) {
        if node.end_byte() <= bytes.start || node.start_byte() >= bytes.end {
            return;
        }
        let start = node.start_position().row;
        let end = node.end_position().row;
        if end.saturating_sub(start) < 2 {
            return;
        }
        ranges.push(FoldRange::new(start, end));
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            collect(child, bytes, ranges);
        }
    }

    let root = tree.root_node();
    let mut ranges = Vec::new();
    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        collect(child, &byte_range, &mut ranges);
    }
    ranges.sort_by_key(|range| range.start_line);
    ranges.dedup_by_key(|range| range.start_line);
    ranges
}
