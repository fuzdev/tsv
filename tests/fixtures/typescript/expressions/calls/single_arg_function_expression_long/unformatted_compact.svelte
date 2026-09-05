<script lang="ts">
	// A lone function-expression argument takes three states: hugged flat, hugged with the
	// function's own group broken, then the argument on its own line. The middle state exists
	// only when the signature has something to break - a parameter list of plain identifiers
	// renders flat inside the hug, so it drops to the last state instead.

	// Boundary: exactly 100 chars - fits, the function stays hugged.
	fnAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA(function () {});

	// Boundary: exactly 101 chars - nothing inside can break, so the argument takes its own line.
	fnAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA(function () {});

	// The `new` twin takes the same three states.
	new AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA(function () {});

	// A body of its own forces the break, so the function stays hugged past width.
	fnAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA(function () {s();});

	// Plain identifier parameters render flat inside the hug, so an over-wide list cannot break
	// there and the whole argument drops to its own line.
	fnAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA(function named(paramOne,paramTwo,paramThree,paramFour,paramFive) {s();});

	// A default value leaves the list breakable, so the same width hugs and breaks the parameters.
	fnAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA(function named(paramOne,paramTwo = 1,paramThree,paramFour,five) {s();});

	// A type annotation reads the same way as a default value.
	fnAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA(function named(paramOne: T,paramTwo,paramThree,paramFour,five) {s();});

	// The `new` twin never renders the list flat, so plain identifiers hug there.
	new AAAAAAAAAAAAAAAAAAAAAAAAAAAA(function named(paramOne,paramTwo,paramThree,paramFour,paramFive) {s();});
</script>
