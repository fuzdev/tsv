<script lang="ts">
	// Fully inline - whole call fits under 100
	const short = fn<Node>(firstArgument, secondArgument);

	// Head on the `=` line fits at 100 - RHS stays, args break, simple type-ref inline
	const reactiveStatementBoundaryFit = nodeAndParentsSatisfyRespectivePredicatesX<LabeledStatement>(
		firstPredicate,
		secondPredicate
	);

	// Head on the `=` line is 101 - break after `=`, simple type-ref inline, args break
	const reactiveStatementBoundaryOver =
		nodeAndParentsSatisfyRespectivePredicatesX<LabeledStatement>(firstPredicate, secondPredicate);

	// Keyword type-arg - break after `=`, `<string>` stays inline
	const reactiveKeywordCheckVariantHere =
		nodeAndParentsSatisfyRespectiveKeywordPredicatesXyzw<string>(firstPredicate, secondPredicate);

	// String-literal type-arg - break after `=`, `<'literalString'>` stays inline (atomic)
	const reactiveStringLiteralArgBoundary =
		someGenericPredicateCheckerForStrLiteral<'literalString'>(firstPredicate, secondPredicate);

	// `this` type-arg - break after `=`, `<this>` stays inline (atomic)
	const reactiveThisTypeArgBoundaryHere =
		someGenericPredicateCheckerForThisTypeArgHereChecker<this>(firstPredicate, secondPredicate);

	// Deliberate over-width: a bare call whose callee alone is long, so `callee<Ref>(` has
	// nothing to break before it - the atomic type-arg stays inline and the head runs past
	// 100 (matches prettier - breaking a single atomic type-arg gains nothing)
	nodeAndParentsSatisfyRespectivePredicatesWithAnExtendedDescriptiveNameThatIsLong<LabeledStatement>(
		firstPredicate,
		secondPredicate
	);

	// Contrast: two simple type-args DO break the `<...>`
	const twoTypeArgList = nodeAndParentsSatisfyRespectivePredicateXyzw<
		AaTypeReference,
		BbTypeReference
	>(firstPredicate);
</script>
