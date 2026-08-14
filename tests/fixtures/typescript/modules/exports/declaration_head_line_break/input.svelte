<script lang="ts" module>
	// `interface`, `type`, `namespace` and `module` are contextual keywords: each
	// heads a declaration only when its NAME follows on the same line (tsc
	// `nextTokenIsIdentifierOrKeywordOnSameLine`). Behind `export` there is no
	// demotion to fall back on, so the valid baseline keeps every name glued to its
	// head keyword.
	export interface I {}
	export type T = number;
	export namespace N {}
	export module M {}
	export declare interface J {}
	export declare type U = number;
</script>
