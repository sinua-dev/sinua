import "@sinua/web/element";
import { bootstrapApplication } from "@angular/platform-browser";
import { provideZonelessChangeDetection } from "@angular/core";
import { AppComponent } from "./app.component";
bootstrapApplication(AppComponent, { providers: [provideZonelessChangeDetection()] });
