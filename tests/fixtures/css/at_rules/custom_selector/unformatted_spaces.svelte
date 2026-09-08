<style>
	/* `@custom-selector :--name <selector-list>`: the `:--name` keeps its case (an
	author-defined name has no canonical casing); the list formats as any selector list,
	comma-joined inline */
	@custom-selector   :--a   h1  ,  h2  ,
		h3  ;
	@custom-selector
		:--KeepName
		div;
	@custom-selector :--b .class1  >  .class2  ~  span  +  a  ,  div    span  ;
	@custom-selector :--c :hover  ,  :focus-within  ,  ::before  ;
	@custom-selector :--d :is(  .class1  ,  .class2  )  ,  :not(  .class3  ):nth-child(  2n  +  1  )  ;
	@custom-selector :--e [  data-attr  =  'value'  ]  ,  [  data-attr  ^=  'value'  i  ]  ;
	@custom-selector :--f :global(  .class1  )  ,  :--a:hover  ;

	/* a defined name is used as a pseudo-class */
	:--a:--c   {
		color  :  red  ;
	}
</style>
