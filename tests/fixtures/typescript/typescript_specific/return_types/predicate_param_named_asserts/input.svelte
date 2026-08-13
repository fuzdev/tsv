<script lang="ts">
	// A parameter may itself be named `asserts`: an ordinary `x is T` predicate
	// outranks the `asserts` modifier, and only the token AFTER the name tells them
	// apart.
	function fn(asserts: unknown): asserts is string {
		return true;
	}

	const arrow = (asserts: unknown): asserts is string => true;

	declare function ambient(asserts: unknown): asserts is string;

	interface Signatures {
		method(asserts: unknown): asserts is string;
		(asserts: unknown): asserts is string;
	}

	class Holder {
		method(asserts: unknown): asserts is string {
			return true;
		}
	}

	// The modifier reading still wins when no `is` follows the name.
	function modifier(value: unknown): asserts value is string {}
	function bare(value: unknown): asserts value {}
	// …and `asserts` alone is an ordinary type reference.
	declare function plain(): asserts;
</script>
