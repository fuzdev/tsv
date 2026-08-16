<script>
	// A unary left operand of `in` / `instanceof` keeps clarity parens — `!a in b` reads as
	// `!(a in b)` to a human, so the parens say which one the author wrote
	(!a) in b;
	(!a) instanceof b;
	// Every unary operator, not just `!`
	(typeof a) in b;
	(-a) instanceof b;
	(void a) in b;
	// The intended-looking form needs its parens by precedence, and keeps them
	!(a in b);
	// An UPDATE expression is not a unary expression — no clarity parens
	a++ in b;
	++a instanceof b;
	// Only the LEFT operand — the right one reads unambiguously
	a in !b;
	a instanceof !b;
	// Only `in` / `instanceof` — other relational operators do not get the parens
	!a < b;
	!a === b;
</script>
