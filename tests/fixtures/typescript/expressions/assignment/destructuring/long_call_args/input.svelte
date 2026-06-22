<script lang="ts">
	// Complex destructuring (>2 props, non-shorthand) + call args
	// When flat exceeds 100, prettier expands destructuring and keeps call args compact

	// 100 boundary — fits flat (tab + 98 content = 100)
	const {
		a_key: aaaaaaaaaaaaa,
		b_key: bbbbbbbbbbbbb,
		c_key: ccccccccccccc
	} = fn1(arg1, { kkkk: vvv });

	// 101 boundary — destructuring expands, call args stay inline
	const {
		d_key: ddddddddddddd,
		e_key: eeeeeeeeeeeee,
		f_key: fffffffffffff
	} = fn2(arg2, { kkkkk: vvv });

	// Short control — fits on one line
	const { g: x, h: y, i: z } = fn3(a, b);
</script>
