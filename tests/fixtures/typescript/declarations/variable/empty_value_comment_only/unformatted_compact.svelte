<script lang="ts">
// An empty object or array holding only a comment hugs the `=` under a binding that could
// break: the comment's forced break stays inside the value and decides nothing at the `=`
const aaaa:Record<string,string>={
// c
};const{bbbb,cccc}={
// c
};const[dddd,eeee]=[
// c
];

// and under a plain identifier
const ffff={
// c
};
</script>
