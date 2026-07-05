<script lang="ts">
	// Contextual type keyword as a namespace name (dispatch gate)
	namespace number {}

	// Keyword in dotted segments: head, tail, and middle positions
	namespace number.a {}
	namespace a.number {}
	namespace a.number.b {}
	namespace A.string {}

	// declare namespace with a keyword name; the keyword still works as a type inside
	declare namespace string {
		interface X {}
		export function foo(s: string): void;
	}
</script>
