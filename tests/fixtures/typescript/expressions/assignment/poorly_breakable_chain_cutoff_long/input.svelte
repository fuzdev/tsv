<script lang="ts">
	// Prettier's `isPoorlyBreakableMemberOrCallChain` gives up on a call whose `printMemberChain`
	// doc is LABELLED `memberChain` - every chain past the cutoff (two groups, or three when the
	// first two merge on a `this` / factory head). Such a chain takes the fluid layout and
	// expands itself; a chain within the cutoff breaks after the `=` and stays whole.

	// 100 - fits
	const aaaa = this.cccc.dddd()?.eeeeeeeeeeee?.ffff?.gggg?.hhhh?.iiii?.jjjj?.includes('kkkkkkkkkk');

	// 101 - three groups, `[this, .cccc] [.dddd()] [?.eeee…includes()]`, and no merge: the chain expands
	const aaaaa = this.cccc
		.dddd()
		?.eeeeeeeeeeee?.ffff?.gggg?.hhhh?.iiii?.jjjj?.includes('kkkkkkkkkk');

	// `this` merges the first two groups only when it is a group of its own - within the cutoff
	const bbbb =
		this.dddd().eeeeeeeeeeeee?.ffff?.gggg?.hhhh?.iiii?.jjjj?.kkkk?.includes('kkkkkkkkkk');

	// a factory head merges the same way, as does a factory property closing the first group
	const cccc =
		Obj.keys(aa).eeeeeeeeeeee?.ffff?.gggg?.hhhh?.iiii?.jjjj?.kkkk?.includes('kkkkkkkkkk');
	const dddd =
		this.Ffff.dddd().eeeeeeeee?.ffff?.gggg?.hhhh?.iiii?.jjjj?.kkkk?.includes('kkkkkkkkk');

	// a call on the base joins the first group: two groups, break after `=`
	const ffff =
		fnfn(aa).eeeeeee?.ffff?.gggg?.hhhh?.iiii?.jjjj?.kkkk?.llll?.mmmm?.then('kkkkkkkkkkk');

	// the assignment twin
	gggg = this.cccc
		.dddd()
		?.eeeeeeeeee?.ffff?.gggg?.hhhh?.iiii?.jjjj?.kkkk?.includes('kkkkkkkkkkkkk');
</script>
