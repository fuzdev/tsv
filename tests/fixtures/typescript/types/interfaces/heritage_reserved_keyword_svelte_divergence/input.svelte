<script lang="ts">
	// A reserved word is never an `IdentifierReference`, so it cannot head a
	// heritage type reference — `void` and `super` are tsc parse errors too
	interface A extends void {}
	interface B extends null {}
	interface C extends true {}
	interface D extends this {}
	interface E extends super {}

	// `class ... implements` shares the list, so it draws the same line
	class F implements void {}
	class G implements null {}
</script>
