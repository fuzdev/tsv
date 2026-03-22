//! Pattern types for public AST (destructuring)

use serde::{Deserialize, Serialize};

use super::expressions::Property;
use super::types::TSTypeAnnotation;
use super::{Expression, Position, SourceLocation};

/// Object pattern for destructuring: `{a, b}`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectPattern {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub properties: Vec<ObjectPatternProperty>,
    #[serde(rename = "typeAnnotation", skip_serializing_if = "Option::is_none")]
    pub type_annotation: Option<TSTypeAnnotation>,
}

/// Object pattern property - either a regular property or a rest element
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ObjectPatternProperty {
    Property(Property),
    RestElement(RestElement),
}

/// Array pattern for destructuring: `[a, b]`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArrayPattern {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub elements: Vec<Option<Expression>>,
    #[serde(rename = "typeAnnotation", skip_serializing_if = "Option::is_none")]
    pub type_annotation: Option<TSTypeAnnotation>,
}

/// Assignment pattern for default values: `a = 1`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssignmentPattern {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: AssignmentPatternLoc,
    pub left: Box<Expression>,
    pub right: Box<Expression>,
}

/// acorn quirk: in function declaration/expression params, when the left side has a
/// typeAnnotation (e.g., `a: number = 0`), `loc.start` becomes a SourceLocation
/// `{start: {line,col}, end: {line,col}}` covering the identifier+typeAnnotation,
/// instead of a plain Position `{line,col}`. Arrow/snippet params use the normal form.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AssignmentPatternLoc {
    /// Normal: `{start: Position, end: Position}`
    Normal(SourceLocation),
    /// Nested: `{start: SourceLocation, end: Position}` — acorn quirk for typed params
    Nested {
        start: SourceLocation,
        end: Position,
    },
}

/// Rest element in destructuring: `...rest`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestElement {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub argument: Box<Expression>,
    #[serde(rename = "typeAnnotation", skip_serializing_if = "Option::is_none")]
    pub type_annotation: Option<TSTypeAnnotation>,
}
