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
 * The refusal of `beneath` in the sibling checkout `checkout` (as the tools spell it,
 * `../svelte`) reaching through a symlink that is absolute or climbs out of it, which no tool
 * follows: the words `sibling_reads` (`scripts/roots/checkout.ts`) throws for the same case.
 */
export const link_out_message = (checkout: string, beneath: string): string =>
	`${checkout}/${beneath} reaches through a symlink that is absolute or climbs out of ${checkout}, which is never followed; make it relative within the checkout, or link the checkout itself instead`;
