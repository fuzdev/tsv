<script lang="ts">
	// a directive trailing an opening brace is inert across member and statement
	// lists — the comment stays on the brace line, the first member formats normally
	class K {
		// prettier-ignore
		a   =   1;
		b = 2;
	}
	enum E {
		// prettier-ignore
		A =   1,
		B = 2
	}
	function f() {
		// prettier-ignore
		fn1(  );
		fn2();
	}
</script>
