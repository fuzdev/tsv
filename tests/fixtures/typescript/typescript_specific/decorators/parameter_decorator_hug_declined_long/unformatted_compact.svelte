<script lang="ts">
	// A decorated sole parameter never hugs: prettier's `hasNotParameterDecorator`
	// declines the hug for ANY decorator, so the parameter list breaks around it
	// instead of welding `(@dec a: {` to the signature.

	class Boundary {
		// 100 chars stays inline, 101 breaks the list
		fn100(@dec a: { aaa: Aaaa; bbb: Bbbb; ccc: Cccc; ddd: Dddd; eee: Eeee; fffff: Ffffff }): void {}
		fn101(@dec a: { aaa: Aaaa; bbb: Bbbb; ccc: Cccc; ddd: Dddd; eee: Eeee; ffffff: Ffffff }): void {}

		// An undecorated sole parameter still hugs
		plain(a: { aaa: Aaaa; bbb: Bbbb; ccc: Cccc; ddd: Dddd; eee: Eeee; ffffffffffff: Ffffff }): void {}

		// Patterns decline the hug too
		pattern(@dec { aaa, bbb, ccc, ddd, eee, fff, ggg, hhh, iii, jjj, kkk, lll, mmm, nnn }: T): void {}
		arrayPattern(@dec [aaa, bbb, ccc, ddd, eee, fff, ggg, hhh, iii, jjj, kkk, lll, mmmm]: T): void {}

		// Any decorator in the list declines it
		multi(@dec1 @dec2 a: { aaa: Aaaa; bbb: Bbbb; ccc: Cccc; ddd: Dddd; eee: Eeee; ff: Ffff }): void {}
	}
</script>
