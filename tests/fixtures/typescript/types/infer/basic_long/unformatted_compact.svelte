<script lang="ts">
// Short - stays on one line (contrast case)
type Flatten<T>=T extends Array<infer Item>?Item:T;

// Long conditional type - extends clause inline, ternary arms break
type LongConditional<T>=T extends Promise<infer LongInferredTypeName>?LongInferredTypeName:never;

// Long extends clause stays inline, ternary arms break
type LongMultiInfer<T>=T extends {first:infer FirstLongType;second:infer SecondLongType}?[FirstLongType,SecondLongType]:never;

// Nested infer - outer and inner conditionals both wrap
type LongNestedInfer<T>=T extends Promise<infer OuterLongType>?OuterLongType extends Promise<infer InnerLongType>?InnerLongType:OuterLongType:T;

// Function params with infer - params break to multiple lines
type LongConstraintInfer<T>=T extends (first:infer FirstParam,...rest:infer RestParams)=>infer ReturnType?[FirstParam,RestParams,ReturnType]:never;

// Chained conditionals in false branch - each level wraps
type ChainedInfer<T>=T extends Promise<infer LongOuterResult>?LongOuterResult:T extends Array<infer LongArrayElement>?LongArrayElement:T;

// Deeply nested (3 levels) - each conditional wraps
type DeeplyNestedInfer<T>=T extends Promise<infer LongOuter>?LongOuter extends Array<infer LongMiddle>?LongMiddle extends object?LongMiddle:never:LongOuter:T;

// Conditional in both branches - true and false both have conditionals
type BothBranchesInfer<T>=T extends Promise<infer LongResultType>?LongResultType extends Error?LongResultType:LongResultType:T extends Array<infer LongElementType>?LongElementType:never;
</script>
