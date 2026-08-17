<script lang="ts">
	// an optional tuple element's required pair, over a conditional's check type
	type A = [((
	// c1
	B) extends C ? D : E)?];

	// the same position over an intersection head
	type F = [((
	// c2
	G) & H)?];

	// an intersection's non-first member, over a conditional's check type
	type I = J & (((
	// c3
	K) extends L ? M : N));

	// the same member position over an intersection head
	type O = P & (((
	// c4
	Q) & R));
</script>
