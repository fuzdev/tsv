<script lang="ts">
	// A type annotation has no break point before the type, so an atomic single type-arg
	// stays inline and the line runs just past 100 (matches prettier via
	// `build_type_arguments_doc_wrapping`). String-literal single type-arg:
	declare const configValueForLiteralArgHere: SomeGenericTypeReferenceConstructorNamed<'literalArg'>;

	// `this` single type-arg - same: stays inline, no break point available
	declare const configValueForThisTypeArgHere: SomeGenericTypeReferenceConstructorForThisValue<this>;

	// Contrast: two type-args still break the `<...>`
	declare const twoTypeArgAnnotation: SomeGenericTypeReferenceConstructorName<
		AaTypeReference,
		BbTypeReference
	>;
</script>
