<!-- `}` inside a regex, string, or comment must not close the {@attach} early -->
<div {@attach a(/[}]/)}></div>
<div {@attach b('}')}></div>
<div {@attach c(/* } */ d)}></div>
