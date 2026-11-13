using Jellyfesh.Services;
using Xunit;

namespace Jellyfesh.Tests;

public class ServicesTests
{
    [Fact]
    public void Bundled_plugins_are_the_packages_in_the_plugin_folder_in_a_stable_order()
    {
        var app = Directory.CreateTempSubdirectory("jf-app-");
        try
        {
            Assert.Empty(BundledPlugins.Find(app.FullName));
            var folder = Directory.CreateDirectory(Path.Combine(app.FullName, BundledPlugins.FolderName));
            foreach (var name in new[] { "org.jellyfesh.subsonic.jfpkg", "org.jellyfesh.jellyfin.jfpkg", "notes.txt" })
            {
                File.WriteAllText(Path.Combine(folder.FullName, name), "x");
            }

            Assert.Equal(
                ["org.jellyfesh.jellyfin.jfpkg", "org.jellyfesh.subsonic.jfpkg"],
                BundledPlugins.Find(app.FullName).Select(Path.GetFileName));
        }
        finally
        {
            app.Delete(recursive: true);
        }
    }
}
