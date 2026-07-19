<div  >
   {#if  a  }
      {#each  items  as  item  }
         {#key  item.id  }
            <Comp  prop={item}  />
         {/key }
      {/each }
   {/if }
</div  >
