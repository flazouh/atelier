//! GitHub's GraphQL answers, as far as atelier reads them. Every field a query may leave out is an
//! `Option` or a default, so a missing one is a value, not a failed parse.

mod helpers;
mod structs;

pub(super) use structs::{
    CommentNode, CommitInfo, CommitNode, ContextNode, Contexts, FileNode, Login, Nodes, Page,
    PullNode, Repo, RequestNode, RestJob, ReviewNode, Root, SearchHit, StateCount, ThreadNode,
};
