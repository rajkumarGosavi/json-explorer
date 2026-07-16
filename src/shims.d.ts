// vue-virtual-scroller 2.0 beta ships no TypeScript types.
declare module "vue-virtual-scroller" {
  import type { DefineComponent } from "vue";
  export const RecycleScroller: DefineComponent<
    Record<string, unknown>,
    Record<string, unknown>,
    unknown
  >;
  export const DynamicScroller: DefineComponent<
    Record<string, unknown>,
    Record<string, unknown>,
    unknown
  >;
  export const DynamicScrollerItem: DefineComponent<
    Record<string, unknown>,
    Record<string, unknown>,
    unknown
  >;
}
