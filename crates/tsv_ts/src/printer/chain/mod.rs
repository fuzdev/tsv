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
//
// 3. **Doc Building** (builder/): Build conditional docs with various break strategies
//    - Member-only chains: use fill() for greedy packing
//    - Chains with calls: use conditionalGroup([oneLine, expanded])
//
// ## Module Organization
//
// - **analysis.rs**: Linearization, grouping, merge decisions
// - **types.rs**: Core data structures (ChainNode, ChainGroup)
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
mod printing;
mod types;

// Re-export public API
pub use analysis::{
    group_chain_nodes, linearize_chain_from_call, linearize_chain_from_member,
    linearize_chain_from_non_null,
};
pub use builder::build_chain_doc;
pub use types::ChainNode;
#[cfg(feature = "buffer_stats")]
pub use types::{ChainGroupNodesVec, ChainGroupVec, ChainNodeVec};
