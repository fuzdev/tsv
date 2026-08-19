<script lang="ts">
	// a redundant parenthesized member - a single-member union (the leading-`|` spelling)
	// needs no pair, so a leading line comment inside it cannot stay "inside"; tsv leads
	// the member with it on its own line (prettier lifts it onto the `&` line,
	// divergent_variant_redundant_paren)
	type Mid = a & (// c1
	| b) & d;

	// nested redundant parens collapse the same way, comment still leading the member
	type Nested = p & ((// c2
	| q)) & r;

	// a ONE-member intersection prints as just its member, so even a real 2-member union's
	// pair is redundant there - the comment leads the member the same way
	type Single = & (// c3
	| a | b);

	// the same one-member intersection at an object type's value
	type SingleValue = { p: & (// c4
	| a | b) };

	// at a type argument
	type SingleTypeArg = q<& (// c5
	| a | b)>;

	// at a parameter annotation
	type SingleParam = (h: & (// c6
	| a | b)) => k;

	// at a tuple element
	type SingleElement = [& (// c7
	| a | b)];

	// under an array suffix, where the pair the position requires opens over the run
	type SingleArray = (& (// c8
	| a | b))[];
</script>
