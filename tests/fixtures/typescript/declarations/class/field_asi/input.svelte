<script lang="ts">
	class C {
		a;
		b = 1;
		// reserved words are valid field names when properly terminated — valid
		export;
		import;
		// a method body self-terminates; the next member needs no `;` — valid
		m() {}
		n;
	}
</script>
