import AppKit
@main struct Tests {
    @MainActor static func main() throws {
        _=NSApplication.shared
        installEditingMenu()
        assert(NSApp.mainMenu?.items.first?.submenu?.items.contains(where:{$0.keyEquivalent=="v"})==true)
        let link=WebLink(id:"fixture",name:"  Example  ",url:"https://example.org/a?q=hello&next=%22quoted%22#part",browser:"/Applications/Fixture Browser.app")
        let form=WebLinkForm(link,browsers:[BrowserChoice(name:"Fixture Browser",path:link.browser!)])
        assert(form.view.frame.width==420 && form.name.frame.width==420 && form.url.frame.width==420)
        assert(form.name.frame.minY>form.url.frame.maxY && form.url.frame.minY>form.browser.frame.maxY)
        assert(form.browser.indexOfSelectedItem==1)
        let selected=try form.value()
        assert(selected.browser==link.browser)
        assert(selected.name=="Example")
        form.browser.selectItem(at:0)
        let defaultChoice=try form.value()
        assert(defaultChoice.browser==nil)
        form.url.stringValue="javascript:alert(1)"
        do {_=try form.value();fatalError("Non-web URL accepted")}catch WebLinkValidation.invalidURL {}
        form.url.stringValue="https://";do {_=try form.value();fatalError("Empty host accepted")}catch WebLinkValidation.invalidURL {}
        form.url.stringValue=link.url;form.name.stringValue=""
        do {_=try form.value();fatalError("Empty name accepted")}catch WebLinkValidation.invalidName {}
        let missing=WebLinkForm(link,browsers:[])
        let missingChoice=try missing.value()
        assert(missingChoice.browser==link.browser,"Editing preserves a missing browser choice")
        let decoded=try JSONDecoder().decode(WebLink.self,from:JSONEncoder().encode(link))
        assert(decoded==link)
        let labels = FolderLabelForm(FolderLabel(alias: "  Work 素材  ", add_on: "  Server  "))
        assert(labels.alias.frame.width == 420 && labels.addOn.frame.width == 420)
        assert(labels.alias.frame.minY > labels.addOn.frame.maxY)
        assert(labels.alias.nextKeyView === labels.addOn)
        let labelValue = try labels.value()
        assert(labelValue == FolderLabel(alias: "Work 素材", add_on: "Server"))
        labels.alias.stringValue = ""; labels.addOn.stringValue = ""
        let cleared = try labels.value()
        assert(cleared == FolderLabel(alias: "", add_on: ""))
        labels.addOn.stringValue = "bad\nlabel"
        do { _ = try labels.value(); fatalError("Multiline label accepted") } catch FolderLabelValidation.invalidText {}
        print("PASS folder alias/add-on form layout, normalization, clearing and validation; no windows shown")
        print("PASS native weblink form geometry, name/URL validation, default/per-link/missing browser choices and JSON; no windows shown or browsers opened")
    }
}
