<script lang="ts">
	// a one-member intersection prints as its member, so the `&` is dropped and its own
	// leading gap is the ENCLOSING gap's - one run, in source order, at that gap's indent

	// a type argument
	type A = q<& /* c1 */ (// d1
	B)>;

	// an object type's value
	type C = { p: & /* c2 */ (// d2
	D) };

	// a union member
	type E = F | (& /* c3 */ (// d3
	G));

	// an optional tuple element
	type H = [(& /* c4 */ (// d4
	I))?];

	// a conditional's check type
	type J = (& /* c5 */ (// d5
	K)) extends L ? M : N;

	// the one-member UNION spelling of the same shape
	type O = q<| /* c6 */ (// d6
	P)>;
</script>
