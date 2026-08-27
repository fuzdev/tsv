// Chain formatting module for TypeScript member chains
//
// This module implements prettier-compatible member chain formatting following
// prettier's linearize→group→conditionalGroup model from member-chain.js.
//
// ## Architecture
//
// 1. **Linearization** (analysis.rs): Flatten nested AST into a flat list of ChainNodes
//    `a().b().c!.d` → [Base(a), Call(), Member(.b), Call(), NonNull(!), Member(.d)]
//
// 2. **Grouping** (analysis.rs): Group nodes by natural break points
//    - First group: base + calls + non-null + numeric accessors + consecutive members
//    - Remaining groups: members* + calls*, break at memberish after call
//    - A member whose gap holds a LINE comment always starts a group (prettier's
//      trailing-comment group split; the factory merge is refused for one too):
//      only a group's first member reaches the chain-level gap emitters, a member
//      printed inside a group can only defer the `//` to the line end
//
// 3. **Doc Building** (builder/): Build conditional docs with various break strategies
//    - Member-only chains: use fill() for greedy packing
//    - Chains with calls: use conditionalGroup([oneLine, expanded])
//
// ## Module Organization
//
// - **analysis.rs**: Linearization, grouping, merge decisions
// - **types.rs**: Core data structures (ChainNode, ChainGroup)
// - **inline_lookups.rs**: which lookups carry a break point, as marked by the chain's
//   PARENT (prettier's `shouldInline` clauses a chain cannot answer for itself)
// - **printing.rs**: Node/group rendering
// - **adapter.rs**: chain-helper methods on the main Printer
// - **builder/**: Doc building logic split into focused submodules
//   - mod.rs: Main build_chain_doc entry point
//   - member_only.rs: Member-only chains using fill()
//   - expansion.rs: Chain expansion analysis helpers
//   - helpers.rs: Shared utilities and ChainPartsBuilder
//
// ## References
// - prettier/src/language-js/print/member-chain.js

mod adapter;
mod analysis;
mod builder;
mod inline_lookups;
mod printing;
mod types;

// Re-export public API
pub use analysis::{
    LinearizeInput, chain_paren_leading_gap, group_chain_nodes, linearize_chain_from_call,
    linearize_chain_from_member, linearize_chain_from_non_null,
};
pub(crate) use analysis::{call_callee_paren_leading_start, tag_paren_leading_start};
pub use builder::build_chain_doc;
pub(crate) use inline_lookups::{InlineLookups, resolve_inline_lookups};
pub(crate) use printing::find_bracket_position;
pub use types::{ChainCall, ChainNode};
#[cfg(feature = "buffer_stats")]
pub use types::{ChainGroupVec, ChainNodeVec};
