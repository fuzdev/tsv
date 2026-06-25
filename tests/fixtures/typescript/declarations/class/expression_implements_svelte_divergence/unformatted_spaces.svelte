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
</script>
