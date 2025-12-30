<style>
	/* @import wraps at 101 chars (we wrap at >100, Prettier waits until >102) */

	/* 100 chars - stays inline (both agree) */
	@import   url('./aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.css')   screen   and   (min-width:   768px);

	/* 101 chars - we wrap, Prettier stays inline */
	@import   url('./aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.css')   screen   and   (min-width:   768px);

	/* 102 chars - both wrap */
	@import   url('./aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.css')   screen   and   (min-width:   768px);
</style>
