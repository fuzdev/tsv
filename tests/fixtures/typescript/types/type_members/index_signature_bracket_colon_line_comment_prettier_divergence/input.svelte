<script lang="ts">
	// A line comment between `]` and the value `:` stays after `]`; the value type
	// drops to the next line, indented one level so it reads as part of this member.
	// Prettier relocates the comment into the brackets, trailing the key type.
	interface A {
		[k: string] // c
			: number;
	}
</script>
