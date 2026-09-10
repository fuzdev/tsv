<script lang="ts">
	// A leading comment in the `=`→value gap does not move where a breaking curried chain
	// breaks: the heads stack under the `=` exactly as they do with no comment.

	// preserved multi-line block
	const a = /* x
y */ ({}) =>
		() =>
			test;

	// indentable block
	const b =
		/**
		 * x
		 */ ({}) =>
			() =>
				test;

	// single-line block
	const c = /* x */ ({}) =>
		() =>
			test;

	// line comment
	const d = // x

			({}) =>
			() =>
				test;

	// a RUN: only the last comment is glued to the head, so only it is owned
	const f = /* x */ /* y */ ({}) =>
		() =>
			test;

	// null control: no comment
	const e =
		({}) =>
		() =>
			test;
</script>
