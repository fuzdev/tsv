<style>
/* Basic scoping */@scope(.card){img{border:1px solid;}}

/* Scope with limit */
@scope(.card) to (.ignore){p{color:blue;}}

/* Named scope root */
@scope(#header){h1{font-size:2rem;}}

/* Scope with multiple selectors */
@scope(.card,.panel){.title{font-weight:bold;}}

/* Scope with complex limit */
@scope(.container) to (.skip,.exclude){a{text-decoration:underline;}}

/* Nested scopes */
@scope(.outer){.outer-content{padding:1rem;}

@scope(.inner){span{font-weight:bold;}}}

/* Scope with descendant selectors */
@scope(article>header){h2{margin:0;}}

/* Scope with pseudo-class */
@scope(.widget:hover){.tooltip{display:block;}}

/* Scope with limit and nested rules */
@scope(.main)to(.footer){section{margin-bottom:2rem;}

nav{position:sticky;}}

/* Scope with attribute selector */
@scope([data-theme='dark']){.background{background:black;color:white;}}
</style>
