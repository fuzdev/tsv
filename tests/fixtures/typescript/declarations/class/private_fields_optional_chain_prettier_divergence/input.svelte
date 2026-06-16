<script lang="ts">
	// Optional chaining to a private field — valid modern JS, but prettier's
	// typescript parser rejects it (see prettier_rejects.txt)
	class C {
		#a = 0;
		access(x?: C) {
			return x?.#a;
		}
	}
</script>
