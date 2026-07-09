<script lang="ts">
	class C {
		// a modifier keyword alone on a line is a field of that name (ASI), not a
		// modifier for the member on the next line — so each pair is two members
		readonly
		a = 1
		private
		b = 2
		async
		d = 4
		// `static` has no [no LineTerminator here] restriction, so it stays a modifier even
		// across a line break (unlike the contextual modifiers above) — one member
		static
		c = 3
	}
</script>
