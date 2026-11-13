/**
 * How the gates and `doctor` name a sibling checkout that is present but can't be used, in one
 * wording whichever tool finds it. Imports nothing, so every library that reads a sibling can
 * load it.
 *
 * @module
 */

/** What a present but unusable sibling checkout is, and the two ways to clear it. */
export const BROKEN_CHECKOUT =
	'a broken checkout, not a machine without one; repair it, or remove it';

/**
 * What clears a present sibling checkout in which git can't read the object a tool asks of it,
 * for each way that happens. The checkout itself may be sound, so it isn't called broken: an
 * export with no history, a repository git refuses for its owner (whose first stderr line the
 * tools print), or a corrupt clone. The ownership fix is to own it: listing it in git's
 * `safe.directory` instead tells git to trust its config, which can run commands as you.
 * Removing it skips what reads it, which a tool that only measures drift is no reason to do.
 */
export const UNREAD_CHECKOUT_REMEDY =
	"make it a clone git can read (an export has no history; a repository git refuses for its owner is made yours, since listing it in git's safe.directory instead trusts its config to run commands as you; a corrupt one is cloned again), or remove it to skip what reads it";

/**
 * The refusal of `beneath` in the sibling checkout `checkout` (as the tools spell it,
 * `../svelte`) reaching through a symlink that is absolute or climbs out of it, which no tool
 * follows: the words `sibling_reads` (`scripts/roots/checkout.ts`) throws for the same case.
 */
export const link_out_message = (checkout: string, beneath: string): string =>
	`${checkout}/${beneath} reaches through a symlink that is absolute or climbs out of ${checkout}, which is never followed; make it relative within the checkout, or link the checkout itself instead`;
