<style>
	/* an ident that merely starts with n is a type selector, not an Nth 2n */
	:foo(nth){color:red}

	/* An+B is case-sensitive (lowercase): N is a type selector, not a bare n */
	:is(N){color:blue}

	/* EVEN is a type selector, not the even keyword */
	:not(EVEN){color:green}

	/* a hyphenated ident starting with n stays a type selector */
	:where(n-th){color:teal}

	/* nom does not match the n An+B production */
	:foo(nom){color:orange}
</style>
