<script lang="ts">
	class Parens {
		// parenthesized decorator expressions ride the full expression grammar
		@(fn().fn1())
		a;

		// unparenthesized decorators use the restricted ident/member/call grammar
		@a.b.c.d()
		b;

		@(fn)()
		c;

		@(a.b)
		d;
	}
</script>
