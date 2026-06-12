<style>
	.class1 {
		/* Empty var() fallback — the trailing comma is significant and preserved */
		color: var(--a,  );
		/* Normal fallback is unaffected */
		background: var(--b,  red);
		/* Nested empty fallback */
		width: var(--c,  var(--d,  ));
	}
</style>
