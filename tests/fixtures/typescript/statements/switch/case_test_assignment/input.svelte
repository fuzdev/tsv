<script>
	// an assignment in a case test takes parens
	switch (x) {
		case (a = 1):
			break;
		case (b += 1):
			break;
		case (c ??= 1):
			break;
	}

	// other expressions follow their own rules: a sequence keeps its parens,
	// a ternary and a comparison stay bare
	switch (x) {
		case (a, b):
			break;
		case a ? b : c:
			break;
		case a === b:
			break;
	}

	// the discriminant is the same position and already parenthesizes
	switch ((a = 1)) {
		case 1:
			break;
	}
</script>
