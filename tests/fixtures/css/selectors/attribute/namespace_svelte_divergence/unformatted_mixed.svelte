<style>
	/* Namespace prefix */[svg|href]{fill:red;}

	/* Universal namespace */[ *|title ]  {color:blue;}

	/* Explicit no namespace */[|lang]{
		font-weight:bold;}

	/* Implicit no namespace (default) */[class] {margin:  0;}

	/* With matcher */[foo|att='val']{color:green;}

	/* With matcher operators */[ svg|href^='https' ]  {text-decoration:underline;}

	/* Combined with other selectors */div[svg|href]{border:1px solid red;}

	/* Multiple attributes */[svg|href][*|title]{opacity:0.5;
	}
</style>
