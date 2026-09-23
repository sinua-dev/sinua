import type {} from "@sinua/web/types/react";
import spec from "../spec.json";
export const Ok = () => <sinua-view pattern="breathing" size={32} spec={spec} inputs={{ micMuted: 1 }} onfxframe={(e) => e.detail.dtMs} />;
