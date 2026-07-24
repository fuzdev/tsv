<script lang="ts">
	// a glued directive is inert — a directive freezes only when it is alone on its
	// line; here it is an ordinary block comment and everything formats normally
	type A = /* prettier-ignore */ (a1 & a2) | (b1 & b2);

	// whole-union position at the `=`
	type B = /* prettier-ignore */ a | b | c;

	// mid-list member positions: leaf, composite, intersection host
	type C = a | /* prettier-ignore */ { x: 1 } | b;
	type D = a | /* prettier-ignore */ ({ y: 2 } & c);
	type E = a & /* prettier-ignore */ { z: 3 } & d;

	// other comments in the run don't change the classification
	type F = a | /* prettier-ignore */ /* other */ { w: 4 } | e;
</script>
