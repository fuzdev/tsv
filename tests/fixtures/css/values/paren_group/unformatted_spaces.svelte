<style>
	div {
		/* a parenthesized group is a value group like any other, so its members take
		every value-level rule they take outside it: a number canonicalizes, a hex
		colour lowercases, a string re-quotes and a unit folds to its canonical case */
		margin: (  1.5  );
		flex: (0.5);
		border-color: (#fff);
		font-family: ('a');
		width: (1px);
		/* the group is one member of the value around it — beside a word, beside
		another group, and nested inside a function */
		padding: a (  1.5  );
		border-width: ( 1.5 )  ( 2.5 );
		height: calc( ( 1.5px ) );
		/* a colon-bearing interior and an operator chain are members too */
		top: (a: 1.5);
		left: (  100vw  -  1.5px  -  2.5px  );
		/* a custom property takes the same path */
		--a: (1.5);
		/* a group holding nothing closes flush, named or not */
		right: (  );
		bottom: x(  );
	}
</style>
