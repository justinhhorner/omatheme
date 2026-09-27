using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;

namespace OmarchyThemes.App.ViewModels;

/// <summary>The large wallpaper preview shown over the theme page.</summary>
public sealed partial class ThemeDetailViewModel
{
    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(PreviewPositionText), nameof(PreviewIsSelected), nameof(PreviewUseText))]
    public partial WallpaperItemViewModel? PreviewItem { get; set; }

    [ObservableProperty]
    public partial bool IsPreviewOpen { get; set; }

    /// <summary>"3 of 8".</summary>
    public string PreviewPositionText => PreviewItem is null ? "" : $"{PreviewIndex + 1} of {Wallpapers.Count}";

    public bool PreviewIsSelected => PreviewItem is not null && PreviewItem == SelectedWallpaper;

    public string PreviewUseText => PreviewIsSelected ? "Selected" : "Use this wallpaper";

    private int PreviewIndex => PreviewItem is null ? -1 : Wallpapers.ToList().IndexOf(PreviewItem);

    // The list is rebuilt when a download swaps remote images for local files; the preview would be stale.
    partial void OnWallpapersChanged(IReadOnlyList<WallpaperItemViewModel> value) => IsPreviewOpen = false;

    partial void OnSelectedWallpaperChanged(WallpaperItemViewModel? value)
    {
        OnPropertyChanged(nameof(PreviewIsSelected));
        OnPropertyChanged(nameof(PreviewUseText));
    }

    /// <summary>Opens the preview on <paramref name="item"/>, or on the selected wallpaper.</summary>
    public void OpenPreview(WallpaperItemViewModel? item = null)
    {
        item ??= SelectedWallpaper ?? Wallpapers.FirstOrDefault();
        if (item is null)
            return;
        PreviewItem = item;
        IsPreviewOpen = true;
    }

    [RelayCommand]
    public void ClosePreview() => IsPreviewOpen = false;

    /// <summary>Next (+1) or previous (-1) wallpaper, wrapping around.</summary>
    public void MovePreview(int delta)
    {
        if (Wallpapers.Count == 0)
            return;
        var index = ((Math.Max(PreviewIndex, 0) + delta) % Wallpapers.Count + Wallpapers.Count) % Wallpapers.Count;
        PreviewItem = Wallpapers[index];
    }

    [RelayCommand]
    private void PreviewNext() => MovePreview(+1);

    [RelayCommand]
    private void PreviewPrevious() => MovePreview(-1);

    /// <summary>Makes the previewed wallpaper the one Apply uses.</summary>
    [RelayCommand]
    private void UsePreviewedWallpaper()
    {
        if (PreviewItem is not null)
            SelectedWallpaper = PreviewItem;
    }
}
