<script lang="ts">
	class A {
		// `readonly` and `static` are the only modifiers an index signature accepts;
		// an accessibility modifier (public/private/protected) is a grammar error tsv
		// rejects at parse — matching acorn and prettier.
		readonly [a: string]: number;
		static [b: number]: string;
	}
</script>
