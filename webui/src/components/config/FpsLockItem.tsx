"use client";

import { useState, useRef, useEffect } from "react";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Pencil, Save, Trash2, X, Lock } from "lucide-react";
import { DeleteGameDialog } from "./DeleteGameDialog";

interface FpsLockItemProps {
  game: string;
  fpsLock: [number, number];
  editingFpsLock: string | null;
  editingFpsLockMin: string;
  editingFpsLockMax: string;
  setEditingFpsLockMin: (value: string) => void;
  setEditingFpsLockMax: (value: string) => void;
  startEditFpsLock: (game: string, fpsLock: [number, number]) => void;
  saveEditedFpsLock: () => void;
  removeFpsLock: (game: string) => void;
}

export function FpsLockItem({
  game,
  fpsLock,
  editingFpsLock,
  editingFpsLockMin,
  editingFpsLockMax,
  setEditingFpsLockMin,
  setEditingFpsLockMax,
  startEditFpsLock,
  saveEditedFpsLock,
  removeFpsLock,
}: FpsLockItemProps) {
  const isEditing = editingFpsLock === game;
  const [showDeleteDialog, setShowDeleteDialog] = useState(false);
  const [isPopupVisible, setIsPopupVisible] = useState(false);
  const minInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (isEditing) {
      setTimeout(() => {
        setIsPopupVisible(true);
      }, 50);

      setTimeout(() => {
        if (minInputRef.current) {
          minInputRef.current.focus();
        }
      }, 350);
    } else {
      setIsPopupVisible(false);
    }
  }, [isEditing]);

  const handleDelete = () => {
    setShowDeleteDialog(true);
  };

  const confirmDelete = () => {
    removeFpsLock(game);
    setShowDeleteDialog(false);
  };

  return (
    <>
      <Card className="border border-border/40 shadow-sm hover:border-border transition-all duration-200">
        <CardContent className="p-0">
          <div className="p-4 space-y-3">
            <div className="flex items-center gap-2">
              <Lock className="h-5 w-5 text-primary" />
              <span className="font-mono text-sm sm:text-base break-words w-full font-medium">
                {game}
              </span>
            </div>
            <div className="pl-7">
              <span className="text-muted-foreground text-sm">
                FPS Lock:{" "}
              </span>
              <span className="text-primary font-medium">
                [{fpsLock[0]}, {fpsLock[1]}]
              </span>
            </div>
            <div className="flex space-x-3 w-full mt-2">
              <Button
                onClick={() => startEditFpsLock(game, fpsLock)}
                variant="secondary"
                size="sm"
                className="h-9 w-9 rounded-full"
              >
                <Pencil className="h-4 w-4" />
              </Button>
              <Button
                onClick={handleDelete}
                variant="destructive"
                size="sm"
                className="h-9 w-9 rounded-full"
              >
                <Trash2 className="h-4 w-4" />
              </Button>
            </div>
          </div>
        </CardContent>
      </Card>

      {isEditing && (
        <div
          className={`fixed top-0 left-0 right-0 z-50 mx-auto max-w-md p-4 transition-opacity duration-300 ${
            isPopupVisible ? "opacity-100" : "opacity-0"
          }`}
        >
          <div
            className="fixed inset-0 bg-black/50 z-40 transition-opacity duration-300"
            style={{ opacity: isPopupVisible ? 1 : 0 }}
            onClick={() => startEditFpsLock("", fpsLock)}
          />

          <Card className="border border-primary shadow-lg z-50 relative">
            <CardContent className="p-4 space-y-3">
              <div className="flex items-center gap-2 w-full">
                <Lock className="h-5 w-5 text-primary" />
                <span className="font-mono text-sm sm:text-base w-full break-words font-medium">
                  {game}
                </span>
              </div>
              <div className="flex gap-2">
                <Input
                  ref={minInputRef}
                  type="number"
                  value={editingFpsLockMin}
                  onChange={(e) => setEditingFpsLockMin(e.target.value)}
                  className="w-full text-sm sm:text-base focus-visible:ring-primary"
                  placeholder="Min FPS"
                />
                <Input
                  type="number"
                  value={editingFpsLockMax}
                  onChange={(e) => setEditingFpsLockMax(e.target.value)}
                  className="w-full text-sm sm:text-base focus-visible:ring-primary"
                  placeholder="Max FPS"
                />
              </div>
              <div className="flex space-x-3 w-full">
                <Button
                  onClick={() => startEditFpsLock("", fpsLock)}
                  variant="destructive"
                  size="sm"
                  className="h-10 w-10 rounded-full"
                >
                  <X className="h-5 w-5" />
                </Button>
                <Button
                  onClick={saveEditedFpsLock}
                  size="sm"
                  className="h-10 w-10 rounded-full"
                >
                  <Save className="h-5 w-5" />
                </Button>
              </div>
            </CardContent>
          </Card>
        </div>
      )}

      <DeleteGameDialog
        isOpen={showDeleteDialog}
        onClose={() => setShowDeleteDialog(false)}
        onConfirm={confirmDelete}
        gameName={game}
      />
    </>
  );
}
