<script lang="ts">
	// A contextual type keyword is an ordinary `Identifier` in a heritage clause —
	// the grammar takes a type REFERENCE there, and primitive-ness is semantic
	interface A extends number {}
	interface B extends string, boolean {}

	// `class ... implements` shares the same list
	class C implements number {}
	class D implements string, boolean {}

	// Class EXPRESSIONS take the same heritage names. (The ANONYMOUS implements-first
	// form is a separate, already-sanctioned acorn `id` divergence — see
	// declarations/class/expression_implements_svelte_divergence)
	const e = class G implements number {};
	const f = class<T> implements string {};

	// A keyword heritage name may carry type arguments and qualify
	interface H extends ns.number {}
	interface I extends any<string> {}
</script>
