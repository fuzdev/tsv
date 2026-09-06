<script lang="ts">
	// The call-object clause (member.js `shouldInline`) reads the last lookup's object through
	// a `!` - prettier's `stripChainElementWrappers` - so a lone lookup off a `!`-wrapped call
	// with arguments inlines, and the width sheds into the arguments
	const aaaa=bbbb.cccccccccccccc(dddddddddddddddddd,dddddddddddddddddd,dddddddddddddddddd)!.eeee;

	// the same lone lookup off a chain that prints as a member chain: the chain expands
	const aaaaa=bbbb.ccccccccccccccccccc().cccc().cccc(dddddddddddddddddd,dddddddddddddddddd)!.eeee;

	// The `memberChain` label does NOT survive a `!`: a lookup whose object is a `!` off an
	// earlier lookup keeps its break point, and the chain stays flat
	const ffffff=bbbb.ccccccccccccccc().cccc().cccc(dddddddddddddddddd,dddddddddddddddddd).eeee!.gg;

	// ...but it does survive a numeric index: the last lookup glues to `[0]` and the pair drops
	const gggggg=bbbb.ccccccccccccc().cccc().cccc(dddddddddddddddddd,dddddddddddddddddd).eeee[0].hh;
</script>
