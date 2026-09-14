<script lang="ts">
	// Cast argument of a postfix update keeps parens — bare a as T++ binds ++ to T
	( a as T )++;
	// Prefix update over a cast keeps parens too
	++( a as T );
	// A satisfies cast binds the same way, at both spellings
	( a satisfies T )++;
	--( a satisfies T );
	// An angle-bracket assertion sits below the operand grammar at both spellings
	( <T>x )++;
	++( <T>x );
	// Redundant parens around a plain reference are stripped
	( a.b )++;
	// A non-null assertion is a left-hand side too, so its pair goes as well
	( a! )++;
	// And the prefix spelling strips the same redundant pair
	++( a.b );
</script>
