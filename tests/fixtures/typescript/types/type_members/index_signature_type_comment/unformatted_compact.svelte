<script lang="ts">
	// Index signature value type with comment
	interface A {
		[key: string]:/* c */B|C;
	}

	// Index signature key type with comment
	interface D {
		[key:/* c */B|C]: string;
	}
</script>
