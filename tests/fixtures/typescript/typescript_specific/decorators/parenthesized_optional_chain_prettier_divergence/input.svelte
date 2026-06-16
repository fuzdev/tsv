<script lang="ts">
	class Crash {
		// a parenthesized optional-chain callee normalizes to parens around the
		// whole expression — valid, but it crashes prettier's estree printer
		@((a?.b)())
		x;
	}
</script>
