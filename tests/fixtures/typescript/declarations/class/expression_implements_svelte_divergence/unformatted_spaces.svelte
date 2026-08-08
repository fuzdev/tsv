<script lang="ts">
	// Anonymous class expression with implements (no extends)
	const   A   =   class   implements   Contract   {};
	const   B   =   class   implements   Contract   {
		prop   :   string   ;
	};

	// Multiple interfaces
	const   C   =   class   implements   Contract1  ,   Contract2   {};

	// Class expression as a `new` callee
	const   D   =   new   ( class   implements   Contract   {} )  ();

	// A contextual type keyword is an ordinary heritage name, and takes the same
	// `id` omission (the shape tsc's own `classImplementsPrimitive.ts` writes)
	const   E   =   class   implements   number   {};
</script>
