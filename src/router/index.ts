import { createRouter, createWebHistory } from "vue-router";
import OpenView from "@/views/OpenView.vue";

const router = createRouter({
  history: createWebHistory(),
  routes: [
    { path: "/", name: "open", component: OpenView },
    {
      path: "/explore",
      name: "explore",
      component: () => import("@/views/ExplorerView.vue"),
    },
  ],
});

export default router;
