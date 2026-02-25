slint::slint! {
    import { GroupBox, Button, LineEdit, CheckBox, HorizontalBox, VerticalBox, ComboBox } from "std-widgets.slint";

    export component ProxyAuthDialog inherits Window {
        title: "Proxy - ScreenHop";
        width: 460px;
        min-height: 340px;
        always-on-top: true;
        default-font-family: root.text_font;

        callback apply(string, string, string, bool, string, string);
        callback cancel();

        in-out property <string> text_font: "";
        in-out property <string> address: "127.0.0.1";
        in-out property <string> port: "7890";
        in-out property <string> protocol: "SOCKS5";
        in-out property <bool> auth_enabled: false;
        in-out property <string> username: "";
        in-out property <string> password: "";

        VerticalBox {
            padding: 12px;
            spacing: 8px;

            GroupBox {
                title: "Server";
                HorizontalBox {
                    spacing: 6px;
                    Text { text: "Address:"; vertical-alignment: center; }
                    LineEdit {
                        text: root.address;
                        edited(txt) => { root.address = txt; }
                    }
                    Text { text: "Port:"; vertical-alignment: center; }
                    LineEdit {
                        text: root.port;
                        width: 60px;
                        edited(txt) => { root.port = txt; }
                    }
                }
            }

            GroupBox {
                title: "Protocol";
                HorizontalBox {
                    alignment: start;
                    ComboBox {
                        model: ["SOCKS5", "SOCKS4", "HTTPS", "HTTP"];
                        current-value: root.protocol;
                        selected(val) => { root.protocol = val; }
                    }
                }
            }

            GroupBox {
                title: "Authentication";

                GridLayout {
                    spacing: 8px;
                    padding: 4px;

                    Row {
                        CheckBox {
                            text: "Enable";
                            checked: root.auth_enabled;
                            toggled => { root.auth_enabled = self.checked; }
                            colspan: 2;
                        }
                    }

                    Row {
                        Text { text: "Username:"; vertical-alignment: center; }
                        LineEdit {
                            text: root.username;
                            enabled: root.auth_enabled;
                            edited(txt) => { root.username = txt; }
                        }
                    }

                    Row {
                        Text { text: "Password:"; vertical-alignment: center; }
                        LineEdit {
                            input-type: InputType.password;
                            text: root.password;
                            enabled: root.auth_enabled;
                            edited(txt) => { root.password = txt; }
                        }
                    }
                }
            }

            HorizontalBox {
                alignment: end;
                spacing: 8px;
                Button { text: "Cancel"; clicked => { root.cancel(); } }
                Button { text: "Save"; primary: true; clicked => { root.apply(root.address, root.port, root.protocol, root.auth_enabled, root.username, root.password); } }
            }
        }
    }

    export component UpdateProgressDialog inherits Window {
        title: "ScreenHop 更新";
        width: 360px;
        min-height: 140px;
        always-on-top: true;
        default-font-family: root.text_font;

        callback cancel();

        in-out property <string> text_font: "";
        in-out property <string> status_text: "正在下载...";
        in-out property <float> progress: 0.0;
        in-out property <bool> can_cancel: true;

        VerticalBox {
            padding: 16px;
            spacing: 12px;

            Text {
                text: root.status_text;
                font-size: 14px;
                wrap: word-wrap;
            }

            Rectangle {
                height: 12px;
                border-radius: 6px;
                background: #e0e0e0;

                Rectangle {
                    x: 0;
                    y: 0;
                    height: 100%;
                    width: parent.width * root.progress;
                    background: #007aff;
                    border-radius: 6px;
                }
            }

            HorizontalBox {
                alignment: end;
                Button {
                    text: "取消";
                    enabled: root.can_cancel;
                    clicked => { root.cancel(); }
                }
            }
        }
    }
}
