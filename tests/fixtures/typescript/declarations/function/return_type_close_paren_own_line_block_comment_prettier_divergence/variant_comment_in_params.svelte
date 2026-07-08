<script lang="ts">
	// A single-line block comment in the `)`→return-type-`:` gap. tsv collapses it
	// inline; prettier relocates it into the parameter list. See README for the full
	// normalization graph.

	// function declaration
	function f(a: string /* c */): string {
		return a;
	}

	// class method
	class C {
		m(a: string /* c */): string {
			return a;
		}
	}

	// type-member method signature
	type T = {
		m(a: string /* c */): string;
	};
</script>
